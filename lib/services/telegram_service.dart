// lib/services/telegram_service.dart — VIDEO TELEGRAM SERVERIDAN
//
// ═══════════════════════════════════════════════════════════════
//  NEGA BU FAYL BOR
// ═══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): xarajatni kamaytirish uchun videolar
// Telegram serveri orqali uzatilsin.
//
// Tizim (batafsil — `worker/src/lib.rs` -> `tg_route` va
// `rust/src/telegram.rs`):
//
//   1. Foydalanuvchi ilovada O'Z Telegram hisobi bilan bir marta
//      kiradi (telefon raqami + kod + kerak bo'lsa parol).
//   2. Qism ochilganda worker obunani tekshiradi va bot videoni
//      yopiq kanaldan foydalanuvchining bot bilan chatiga yuboradi.
//   3. Rust yadrosi faylni o'sha chatdan oladi va telefonda mahalliy
//      manba (`127.0.0.1/tg/...`) ochadi. Pleyer shundan o'qiydi —
//      onlayn ko'rishda baytlar faqat XOTIRADA, yuklab olishda esa
//      hozirgidek 1 MB lab shifrlanib saqlanadi.
//
// Telegram ulanmagan, video kanalda yo'q yoki biror narsa ishlamasa
// — hamma joyda odatdagi worker (B2) yo'li ishlaydi. Ya'ni bu xizmat
// faqat QO'SHIMCHA manba: uning xatosi videoni hech qachon to'xtatmaydi.

import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;

import 'auth_service.dart';
import 'rust_bridge.dart';

typedef _InitC = Pointer<Utf8> Function(Pointer<Utf8>, Int32, Pointer<Utf8>);
typedef _InitDart = Pointer<Utf8> Function(Pointer<Utf8>, int, Pointer<Utf8>);
typedef _NoArgC = Pointer<Utf8> Function();
typedef _StrArgC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _RouteC = Int32 Function(Pointer<Utf8>, Int32);
typedef _RouteDart = int Function(Pointer<Utf8>, int);
typedef _FreeC = Void Function(Pointer<Utf8>);
typedef _FreeDart = void Function(Pointer<Utf8>);

DynamicLibrary _openLib() => Platform.isAndroid
    ? DynamicLibrary.open('librust_core.so')
    : DynamicLibrary.process();

/// Rust qaytargan satrni o'qib, xotirasini bo'shatadi.
String _take(DynamicLibrary lib, Pointer<Utf8> p) {
  if (p == nullptr) return '';
  final s = p.toDartString();
  lib.lookupFunction<_FreeC, _FreeDart>('rust_free_string')(p);
  return s;
}

Map<String, dynamic> _json(String s) {
  try {
    final v = jsonDecode(s);
    if (v is Map<String, dynamic>) return v;
  } catch (_) {}
  return {'error': s.isEmpty ? 'Javob yo\'q' : s};
}

/// Bitta satr argumentli, TARMOQQA chiqadigan Rust funksiyasini
/// alohida isolate'da chaqiradi (aks holda UI qotib qolardi).
Future<Map<String, dynamic>> _callBlocking(String fn, String arg) {
  return Isolate.run(() {
    final lib = _openLib();
    final f = lib.lookupFunction<_StrArgC, _StrArgC>(fn);
    final a = arg.toNativeUtf8();
    try {
      return _json(_take(lib, f(a)));
    } finally {
      malloc.free(a);
    }
  });
}

/// Kirish bosqichining natijasi.
class TgLoginStep {
  final bool done;
  final bool needPassword;
  final String hint;
  final String? error;

  const TgLoginStep({
    this.done = false,
    this.needPassword = false,
    this.hint = '',
    this.error,
  });

  factory TgLoginStep.fromJson(Map<String, dynamic> j) => TgLoginStep(
        done: j['ok'] == true,
        needPassword: j['password'] == true,
        hint: (j['hint'] as String?) ?? '',
        error: j['error'] as String?,
      );
}

class TelegramService extends ChangeNotifier {
  TelegramService._();
  static final TelegramService instance = TelegramService._();

  DynamicLibrary? _lib;
  bool _started = false;

  /// Server Telegram orqali video berishga tayyormi (sirlar qo'yilgan).
  bool _serverEnabled = false;

  /// Kirish boti (`/start <token>` shunga yuboriladi).
  String _bot = '';

  /// Server videolarni Telegram'dan beradimi (kanal sozlanganmi).
  /// Sozlama kelguncha `true` — aks holda ilova ochilgan zahoti
  /// ko'rilgan birinchi qism bekorga B2'dan ketardi.
  bool _video = true;
  bool _configured = false;
  bool _authorized = false;

  /// Shu seansda Telegram'da YO'Q deb bilingan fayllar (qayta-qayta
  /// so'ralmasin).
  final Set<String> _missing = {};

  /// Shu fayl uchun keyingi so'rov bot chatiga QAYTA yuborishni
  /// talab qiladi (eski xabar o'chirilgan bo'lishi mumkin).
  final Set<String> _force = {};

  bool get serverEnabled => _serverEnabled;

  /// Telegram hisobi ulanganmi.
  bool get authorized => _authorized;

  /// Videolar Telegram'dan olinadimi.
  bool get active => _authorized && _configured && _video;

  String get _dir {
    final root = RustCore.instance.rootDirPath ?? '';
    return root.isEmpty ? '' : '$root/tg';
  }

  void _apply(Map<String, dynamic> j) {
    if (j['error'] != null) return;
    _configured = j['configured'] == true;
    _authorized = j['authorized'] == true;
  }

  /// Ilova ishga tushganda (`main`) chaqiriladi. Tarmoqqa chiqmaydi:
  /// saqlangan sessiya o'qiladi, serverdan sozlama esa fon'da olinadi.
  Future<void> start() async {
    if (_started) return;
    final dir = _dir;
    if (dir.isEmpty) return;
    try {
      _lib = _openLib();
      _started = true;
      _init(dir, 0, '');
    } catch (e) {
      debugPrint('Telegram: yadro topilmadi: $e');
      return;
    }
    notifyListeners();
    unawaited(refreshConfig());
  }

  void _init(String dir, int apiId, String apiHash) {
    final lib = _lib;
    if (lib == null) return;
    final f = lib.lookupFunction<_InitC, _InitDart>('rust_tg_init');
    final d = dir.toNativeUtf8();
    final h = apiHash.toNativeUtf8();
    try {
      _apply(_json(_take(lib, f(d, apiId, h))));
    } finally {
      malloc.free(d);
      malloc.free(h);
    }
  }

  /// `api_id`/`api_hash` ni serverdan oladi. Sessiya shart emas:
  /// ilovaga telefon raqami bilan KIRISHning o'zi shu qiymatlar
  /// bilan bo'ladi. `true` — Telegram orqali kirish mumkin.
  Future<bool> refreshConfig() async {
    if (!_started) await start();
    if (!_started) return false;
    try {
      final r = await http
          .get(Uri.parse('$kApiBase/api/tg/config'))
          .timeout(const Duration(seconds: 15));
      if (r.statusCode != 200) return _configured;
      final j = jsonDecode(r.body) as Map<String, dynamic>;
      _serverEnabled = j['enabled'] == true;
      _bot = (j['bot'] as String?) ?? _bot;
      _video = j['video'] != false;
      if (_serverEnabled) {
        final id = (j['api_id'] as num?)?.toInt() ?? 0;
        final hash = (j['api_hash'] as String?) ?? '';
        if (id > 0 && hash.isNotEmpty) _init(_dir, id, hash);
      }
      notifyListeners();
    } catch (_) {
      // Tarmoq yo'q — saqlangan sozlama bilan ishlayveradi.
    }
    return _configured;
  }

  // ── KIRISH ──────────────────────────────────────────────────

  Future<TgLoginStep> requestCode(String phone) async {
    await refreshConfig();
    if (!_configured) {
      return const TgLoginStep(
          error: 'Telegram orqali ko\'rish hozircha yoqilmagan');
    }
    final j = await _callBlocking('rust_tg_request_code', phone);
    return TgLoginStep.fromJson(j);
  }

  Future<TgLoginStep> signIn(String code) async {
    final step =
        TgLoginStep.fromJson(await _callBlocking('rust_tg_sign_in', code));
    if (step.done) _afterLogin();
    return step;
  }

  Future<TgLoginStep> checkPassword(String password) async {
    final step = TgLoginStep.fromJson(
        await _callBlocking('rust_tg_check_password', password));
    if (step.done) _afterLogin();
    return step;
  }

  /// Ilovaga KIRISH: Telegram hisobi ulangach, foydalanuvchi nomidan
  /// botga `/start <token>` yuboriladi (`rust_tg_start_bot` izohi).
  Future<String?> startBot(String token) async {
    if (_bot.isEmpty) await refreshConfig();
    if (_bot.isEmpty) return 'Bot nomi serverdan olinmadi';
    final bot = _bot;
    final j = await Isolate.run(() {
      final lib = _openLib();
      final f = lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>),
          Pointer<Utf8> Function(
              Pointer<Utf8>, Pointer<Utf8>)>('rust_tg_start_bot');
      final b = bot.toNativeUtf8();
      final t = token.toNativeUtf8();
      try {
        return _json(_take(lib, f(b, t)));
      } finally {
        malloc.free(b);
        malloc.free(t);
      }
    });
    return j['ok'] == true ? null : (j['error'] as String? ?? 'Xato');
  }

  void _afterLogin() {
    _authorized = true;
    _missing.clear();
    notifyListeners();
  }

  Future<void> logout() async {
    if (!_started) return;
    await Isolate.run(() {
      final lib = _openLib();
      _take(lib, lib.lookupFunction<_NoArgC, _NoArgC>('rust_tg_logout')());
    });
    _authorized = false;
    _missing.clear();
    notifyListeners();
  }

  // ── VIDEO MANBASI ───────────────────────────────────────────

  /// Worker manzilidan fayl nomi (Rust'dagi kesh kaliti bilan bir
  /// xil: `.../api/image/<nom>?...` -> `<nom>`).
  static String fileNameOf(String url) {
    final noQuery = url.split('?').first;
    final last = noQuery.split('/').last;
    final ok = RegExp(r'^[A-Za-z0-9._-]+$');
    return ok.hasMatch(last) && last != '.' && last != '..' ? last : '';
  }

  String _playUrl(String name) {
    final lib = _lib;
    if (lib == null) return '';
    final f = lib.lookupFunction<_StrArgC, _StrArgC>('rust_tg_play_url');
    final n = name.toNativeUtf8();
    try {
      return _take(lib, f(n));
    } finally {
      malloc.free(n);
    }
  }

  void _route(String name, int msgId) {
    final lib = _lib;
    if (lib == null) return;
    final f = lib.lookupFunction<_RouteC, _RouteDart>('rust_tg_route');
    final n = name.toNativeUtf8();
    try {
      f(n, msgId);
    } finally {
      malloc.free(n);
    }
  }

  /// Videoni Telegram'dan ko'rish/yuklash uchun tayyorlaydi va
  /// pleyer manzilini qaytaradi. Telegram ishlatib bo'lmasa `null`
  /// — chaqiruvchi odatdagi worker yo'liga o'tadi.
  ///
  /// HECH QACHON xato tashlamaydi.
  Future<String?> prepare(String url) async {
    try {
      return await _prepare(url).timeout(const Duration(seconds: 20));
    } catch (e) {
      debugPrint('Telegram: tayyorlanmadi: $e');
      return null;
    }
  }

  /// Rust yadrosidagi holatni o'qiydi (tarmoqsiz). Sessiyani
  /// Telegram bekor qilgan bo'lsa yadro buni o'zi belgilaydi.
  void _syncStatus() {
    final lib = _lib;
    if (lib == null) return;
    final was = _authorized;
    _apply(_json(
        _take(lib, lib.lookupFunction<_NoArgC, _NoArgC>('rust_tg_status')())));
    if (was != _authorized) notifyListeners();
  }

  Future<String?> _prepare(String url) async {
    _syncStatus();
    if (!active) return null;
    final name = fileNameOf(url);
    if (name.isEmpty || _missing.contains(name)) return null;

    // Avval bog'langan va hali ishlayotgan bo'lsa — tarmoqsiz.
    if (!_force.contains(name)) {
      final ready = _playUrl(name);
      if (ready.isNotEmpty) return ready;
    }

    final s = AuthService.instance.sessionToken;
    if (s == null) return null;
    final force = _force.remove(name);
    final r = await http.post(
      Uri.parse('$kApiBase/api/tg/deliver'),
      headers: {
        'Authorization': 'Bearer $s',
        'Content-Type': 'application/json',
      },
      body: jsonEncode({'file': name, 'force': force}),
    );
    if (r.statusCode == 404) {
      // Bu qism hali kanalga yuklanmagan — B2'dan ko'riladi.
      _missing.add(name);
      return null;
    }
    if (r.statusCode != 200) return null;
    final msgId =
        ((jsonDecode(r.body) as Map<String, dynamic>)['msg_id'] as num?)
                ?.toInt() ??
            0;
    if (msgId <= 0) return null;
    _route(name, msgId);
    final u = _playUrl(name);
    return u.isEmpty ? null : u;
  }

  /// Telegram manbasi ishlamadi (masalan foydalanuvchi bot chatidagi
  /// xabarni o'chirgan). Bog'lanish olib tashlanadi va keyingi safar
  /// bot videoni QAYTA yuboradi.
  void invalidate(String url) {
    final name = fileNameOf(url);
    if (name.isEmpty) return;
    _route(name, 0);
    _force.add(name);
  }
}
