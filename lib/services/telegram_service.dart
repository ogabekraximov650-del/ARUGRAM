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
// ── SHIFRLASH (AES-128-CTR) ─────────────────────────────────
//
// Ilova yuklaydigan har bir fayl yuklanish paytida shifrlanadi
// (`rust/src/telegram.rs` -> "SHIFRLAB YUKLASH"). Kalit serverga
// yoziladi va faylni ko'rish ruxsati bor odamga `/api/tg/deliver`
// javobida (`keys`) keladi — shu yerda Rust yadrosiga beriladi
// (`_applyKeys`), yadro esa Telegram'dan kelgan baytlarni o'zi
// ochadi.
//
// Telegram ulanmagan, video kanalda yo'q yoki biror narsa ishlamasa
// — hamma joyda odatdagi worker (B2) yo'li ishlaydi. Ya'ni bu xizmat
// faqat QO'SHIMCHA manba: uning xatosi videoni hech qachon to'xtatmaydi.

import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:ffi/ffi.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:http/http.dart' as http;

import 'app_build.dart';
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

  /// Kod QAYERGA yuborilgani (`via`: app/sms/call/email/...,
  /// `length`, `next`, `timeout`, `at`) — Rust `sent_info`.
  final Map<String, dynamic> sent;

  /// Kod so'rovidan keyin Telegram kodsiz kiritgan (kamdan-kam).
  final bool loggedIn;

  /// Telegram aytgan kutish (soniya; 0 — yo'q).
  final int wait;

  const TgLoginStep({
    this.done = false,
    this.needPassword = false,
    this.hint = '',
    this.error,
    this.sent = const {},
    this.loggedIn = false,
    this.wait = 0,
  });

  factory TgLoginStep.fromJson(Map<String, dynamic> j) => TgLoginStep(
        done: j['ok'] == true,
        needPassword: j['password'] == true,
        hint: (j['hint'] as String?) ?? '',
        error: j['error'] as String?,
        sent: (j['sent'] as Map?)?.cast<String, dynamic>() ?? const {},
        loggedIn: j['ok'] == true && j['sent'] == null,
        wait: (j['wait'] as num?)?.toInt() ?? 0,
      );
}

class TelegramService extends ChangeNotifier with WidgetsBindingObserver {
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

  /// Yopiq video kanali (`-100...`). Admin videoni shu yerga yuklaydi.
  int _channel = 0;

  /// Admin videoni ilovadan to'g'ridan-to'g'ri kanalga yuklay oladimi.
  bool get canUpload => _authorized && _configured && _channel != 0;
  bool _configured = false;
  bool _authorized = false;

  /// Shu seansda Telegram'da YO'Q deb bilingan fayllar (qayta-qayta
  /// so'ralmasin).
  final Set<String> _missing = {};

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
    final was = _authorized;
    _configured = j['configured'] == true;
    _authorized = j['authorized'] == true;
    _port = (j['port'] as num?)?.toInt() ?? _port;
    if (was && !_authorized) _onSessionLost();
  }

  // ── SESSIYA UZILDI ──────────────────────────────────────────
  //
  // TALAB (foydalanuvchi): "Telegram'dan sessiya uzilishi bilan
  // ilova yangi sessiya yaratib, raqam yozadigan oynani chiqarsin".
  //
  // Rust yadrosi Telegram "sessiya yo'q" deganini sezishi bilan
  // sessiyani tashlaydi (`check_dead`). Bu yerda:
  //   * `authorized` o'chadi — `AuthGate` darhol raqam oynasini
  //     ko'rsatadi (ochiq sahifalarni ham yopadi);
  //   * bot chatida qolgan nusxalar QAYTA KIRILGACH tozalanadi —
  //     o'lgan sessiya ularni o'chira olmaydi.
  //
  // ILOVA SESSIYASI TELEGRAM SESSIYASIGA BOG'LANGAN: Telegram
  // sessiyasi uzilsa ilova hisobidan ham chiqiladi (ma'lumotlar
  // navbati hisob papkasida qoladi va qayta kirilganda yuboriladi).
  // Qayta kirishda yangi ilova sessiyasi Telegram imzosi bilan
  // botsiz ochiladi (`urlAuth`).
  void _onSessionLost() {
    _holders.clear();
    _readPending = true;
    _missing.clear();
    notifyListeners();
    if (AuthService.instance.isLoggedIn) {
      unawaited(AuthService.instance.logout());
    }
  }

  Timer? _statusTimer;

  /// Sessiya oxirgi marta qachon tekshirilgan.
  DateTime _checkedAt = DateTime.fromMillisecondsSinceEpoch(0);
  Future<void>? _checking;

  /// Sessiya hali tirikmi — Telegram'ga bitta arzon so'rov
  /// (`rust_tg_check_session`). Tez-tez chaqirilsa ham [every] ichida
  /// bir marta. Tarmoq yo'q bo'lsa holat o'zgarmaydi.
  Future<void> checkSession(
      {Duration every = const Duration(seconds: 30)}) {
    if (!_started || !_authorized) return Future.value();
    final running = _checking;
    if (running != null) return running;
    if (DateTime.now().difference(_checkedAt) < every) return Future.value();
    _checkedAt = DateTime.now();
    final f = () async {
      try {
        final j = await Isolate.run(() {
          final lib = _openLib();
          return _json(_take(
              lib,
              lib.lookupFunction<_NoArgC, _NoArgC>(
                  'rust_tg_check_session')()));
        });
        _apply(j);
      } catch (_) {}
    }();
    _checking = f.whenComplete(() => _checking = null);
    return _checking!;
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // Ilovaga qaytildi — foydalanuvchi shu orada Telegram'da
    // "Qurilmalar"dan chiqargan bo'lishi mumkin.
    if (state == AppLifecycleState.resumed) unawaited(checkSession());
  }

  /// Rust yadrosidagi mahalliy Telegram manbasi porti.
  int _port = 0;

  /// Ilova ishga tushganda (`main`) chaqiriladi. Tarmoqqa chiqmaydi:
  /// saqlangan sessiya o'qiladi, serverdan sozlama esa fon'da olinadi.
  Future<void> start() async {
    if (_started) return;
    final dir = _dir;
    if (dir.isEmpty) return;
    try {
      _lib = _openLib();
      _started = true;
      await _setDevice();
      _init(dir, 0, '');
    } catch (e) {
      debugPrint('Telegram: yadro topilmadi: $e');
      return;
    }
    notifyListeners();
    unawaited(refreshConfig().then((_) async {
      await checkSession(every: Duration.zero);
      // Oldingi seansdan (ilova yiqilgan bo'lsa) qolgan nusxalar.
      _readPending = true;
      return _markReadIfPending();
    }));
    _watchConnectivity();
    WidgetsBinding.instance.addObserver(this);
    // Yadro sessiya o'lganini (masalan pleyer o'qiyotganda) o'zi
    // sezadi — holat xotiradan o'qiladi, tarmoqqa chiqilmaydi.
    _statusTimer ??=
        Timer.periodic(const Duration(seconds: 3), (_) => _syncStatus());
  }

  /// Telegram'ga o'zini haqiqiy telefon sifatida tanitadi
  /// (`rust_tg_set_device` izohi) — ulanishdan OLDIN.
  Future<void> _setDevice() async {
    final lib = _lib;
    if (lib == null) return;
    var model = '';
    var system = '';
    try {
      final m = await const MethodChannel('aru/signature')
          .invokeMapMethod<String, dynamic>('device');
      model = '${m?['model'] ?? ''}';
      system = '${m?['system'] ?? ''}';
    } catch (_) {}
    final app = kAppVersion.isEmpty ? '1.0' : kAppVersion.split('+').first;
    final f = lib.lookupFunction<
        Void Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>),
        void Function(Pointer<Utf8>, Pointer<Utf8>,
            Pointer<Utf8>)>('rust_tg_set_device');
    final a = (model.isEmpty ? 'ARUGRAM' : model).toNativeUtf8();
    final b = system.toNativeUtf8();
    final c = 'ARUGRAM $app'.toNativeUtf8();
    try {
      f(a, b, c);
    } finally {
      malloc.free(a);
      malloc.free(b);
      malloc.free(c);
    }
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
  /// Sozlama telefonda shuncha vaqt saqlanadi — har ishga tushishda
  /// worker'ga so'rov ketmasin (foydalanuvchi talabi: kamroq so'rov).
  static const _configTtl = Duration(hours: 12);

  void _applyConfig(Map<String, dynamic> j) {
    _serverEnabled = j['enabled'] == true;
    _bot = (j['bot'] as String?) ?? _bot;
    _setBot(_bot);
    _video = j['video'] != false;
    _channel = (j['channel'] as num?)?.toInt() ?? 0;
    if (_serverEnabled) {
      final id = (j['api_id'] as num?)?.toInt() ?? 0;
      final hash = (j['api_hash'] as String?) ?? '';
      if (id > 0 && hash.isNotEmpty) _init(_dir, id, hash);
    }
  }

  Future<bool> refreshConfig({bool force = false}) async {
    if (!_started) await start();
    if (!_started) return false;
    if (!force) {
      final cached = RustCore.instance.getCachedList('tg_config');
      final c = (cached != null && cached.isNotEmpty) ? cached.first : null;
      final at = (c?['at'] as num?)?.toInt() ?? 0;
      final fresh = DateTime.now().millisecondsSinceEpoch - at <
          _configTtl.inMilliseconds;
      if (c != null && fresh && c['enabled'] == true) {
        _applyConfig(c);
        notifyListeners();
        return _configured;
      }
    }
    try {
      final r = await http
          .get(Uri.parse('$kApiBase/api/tg/config'))
          .timeout(const Duration(seconds: 15));
      if (r.statusCode != 200) return _configured;
      final j = jsonDecode(r.body) as Map<String, dynamic>;
      _applyConfig(j);
      RustCore.instance.saveListCache('tg_config', [
        {...j, 'at': DateTime.now().millisecondsSinceEpoch}
      ]);
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
    final step = TgLoginStep.fromJson(j);
    if (step.loggedIn) _afterLogin();
    return step;
  }

  // ── QR ORQALI KIRISH ────────────────────────────────────────

  /// QR uchun token: `{"url","expires"}`, tasdiqlangan bo'lsa
  /// `{"ok":true}`, parol kerak bo'lsa `{"password":true,"hint"}`.
  Future<Map<String, dynamic>> qrToken() async {
    await refreshConfig();
    final j = await Isolate.run(() {
      final lib = _openLib();
      return _json(_take(
          lib, lib.lookupFunction<_NoArgC, _NoArgC>('rust_tg_qr_token')()));
    });
    if (j['ok'] == true) _afterLogin();
    return j;
  }

  /// QR boshqa qurilmada tasdiqlandimi (tarmoqsiz).
  bool qrAccepted() {
    final lib = _lib;
    if (lib == null) return false;
    return lib.lookupFunction<Int32 Function(), int Function()>(
            'rust_tg_qr_accepted')() ==
        1;
  }

  /// Kodni QAYTA yuborish — Telegram keyingi usulni tanlaydi
  /// (SMS, qo'ng'iroq...).
  Future<TgLoginStep> resendCode() async {
    final j = await Isolate.run(() {
      final lib = _openLib();
      return _json(_take(
          lib, lib.lookupFunction<_NoArgC, _NoArgC>('rust_tg_resend_code')()));
    });
    final step = TgLoginStep.fromJson(j);
    if (step.loggedIn) _afterLogin();
    return step;
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
    if (_bot.isEmpty) await refreshConfig(force: true);
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

  /// IP manzil bo'yicha davlat (`UZ`) — Telegram'ning o'zidan
  /// (`help.getNearestDc`). Aniqlanmasa `null`.
  Future<String?> nearestCountry() async {
    if (!_started) return null;
    try {
      final j = await Isolate.run(() {
        final lib = _openLib();
        return _json(_take(
            lib,
            lib.lookupFunction<_NoArgC, _NoArgC>(
                'rust_tg_nearest_country')()));
      });
      final c = j['country'] as String?;
      return c == null || c.isEmpty ? null : c;
    } catch (_) {
      return null;
    }
  }

  /// Botsiz kirish: foydalanuvchi nomidan [url] ga Telegram Login
  /// so'raladi (`rust_tg_url_auth`). Javob — Telegram imzolagan
  /// manzil yoki `null` ([onError] ga sababi).
  Future<String?> urlAuth(String url, {void Function(String)? onError}) async {
    final j = await Isolate.run(() {
      final lib = _openLib();
      final f = lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>),
          Pointer<Utf8> Function(Pointer<Utf8>)>('rust_tg_url_auth');
      final u = url.toNativeUtf8();
      try {
        return _json(_take(lib, f(u)));
      } finally {
        malloc.free(u);
      }
    });
    final signed = j['url'] as String?;
    if (j['ok'] == true && signed != null && signed.isNotEmpty) return signed;
    onError?.call(j['error'] as String? ?? 'Xato');
    return null;
  }

  // ── FAYL YUKLASH (hammasi ilova orqali) ─────────────────────
  //
  // TALAB (foydalanuvchi): "worker orqali umuman fayl o'tmasin —
  // Telegram'ga yuklanadigan va yuklab olinadigan barcha narsalar
  // ilovaning o'zidan o'tkazilsin" hamda "fayllar HUJJAT emas, ODDIY
  // ko'rinishda yuborilsin".
  //
  //   * ADMIN — to'g'ridan-to'g'ri yopiq kanalga (u kanalda admin),
  //     so'ng `/api/tg/admin/file` nomni postga bog'laydi;
  //   * BOSHQALAR — o'z bot chatiga; bot uni kanalga nusxalaydi
  //     (worker'dagi `tg_user_media`), ilova esa `/api/tg/claim` bilan
  //     tayyor bo'lishini kutadi.
  //
  // Ikkala yo'lda ham fayl baytlari worker'dan o'tmaydi; hajm
  // chegarasi Telegram'niki (2 GB, Premium'da 4 GB).

  /// Oxirgi yuklangan fayllarning ochish kalitlari (hex) — qism
  /// qo'shish ekrani kalitni `epizod_db.key_*` ga yozadi.
  final Map<String, String> _uploadedKeys = {};

  /// [fileName] yuklanganda berilgan kalit (bo'lmasa bo'sh satr).
  String keyFor(String fileName) => _uploadedKeys[fileName] ?? '';

  /// Serverdan kelgan kalitlarni Rust yadrosiga beradi.
  void _applyKeys(Object? keys) {
    final lib = _lib;
    if (lib == null || keys is! Map || keys.isEmpty) return;
    final f = lib.lookupFunction<Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('rust_tg_set_keys');
    final j = jsonEncode(keys).toNativeUtf8();
    try {
      f(j);
    } finally {
      malloc.free(j);
    }
  }

  /// Faylni Telegram'ga yuklaydi (yo'lda AES-128-CTR bilan
  /// shifrlanadi). Muvaffaqiyatda `null`, aks holda xato matni.
  /// [onProgress] — (yuborilgan, jami) baytlar. Kalit — [keyFor].
  Future<String?> uploadFile(
    String path,
    String fileName,
    String mime, {
    void Function(int sent, int total)? onProgress,
  }) async {
    if (!_authorized) return 'Telegram hisobi ulanmagan';
    if (_channel == 0) await refreshConfig(force: true);
    if (_channel == 0) return 'Kanal sozlanmagan';
    final admin = AuthService.instance.user?.isAdmin ?? false;
    final lib = _lib;
    if (lib == null) return 'Telegram ishga tushmagan';

    final start = lib.lookupFunction<
        Pointer<Utf8> Function(
            Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>, Int64),
        Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>,
            int)>('rust_tg_upload_start');
    final status = lib.lookupFunction<Pointer<Utf8> Function(Uint64),
        Pointer<Utf8> Function(int)>('rust_tg_upload_status');

    final p = path.toNativeUtf8();
    final n = fileName.toNativeUtf8();
    final m = mime.toNativeUtf8();
    Map<String, dynamic> started;
    try {
      // 0 — bot chatiga (kanalga admin bo'lmagan foydalanuvchi).
      started = _json(_take(lib, start(p, n, m, admin ? _channel : 0)));
    } finally {
      malloc.free(p);
      malloc.free(n);
      malloc.free(m);
    }
    final job = (started['job'] as num?)?.toInt() ?? 0;
    if (job <= 0) return (started['error'] as String?) ?? 'Yuklash boshlanmadi';

    // Yuklash Rust'da fon'da ketadi — holatni so'rab turamiz.
    var msgId = 0;
    var key = '';
    while (true) {
      await Future<void>.delayed(const Duration(milliseconds: 400));
      final st = _json(_take(lib, status(job)));
      final sent = (st['sent'] as num?)?.toInt() ?? 0;
      final total = (st['total'] as num?)?.toInt() ?? 0;
      if (total > 0) onProgress?.call(sent, total);
      if (st['done'] == true) {
        final err = st['error'];
        if (err is String && err.isNotEmpty) return err;
        msgId = (st['msg_id'] as num?)?.toInt() ?? 0;
        key = (st['key'] as String?) ?? '';
        break;
      }
    }
    _missing.remove(fileName);
    if (key.isNotEmpty) _uploadedKeys[fileName] = key;
    final s = AuthService.instance.sessionToken;
    if (s == null) return 'Ilova hisobiga kirilmagan';

    if (admin) {
      if (msgId <= 0) return 'Kanal xabari raqami olinmadi';
      for (var attempt = 0; attempt < 3; attempt++) {
        try {
          final r = await http
              .post(
                Uri.parse('$kApiBase/api/tg/admin/file'),
                headers: {
                  'Authorization': 'Bearer $s',
                  'Content-Type': 'application/json',
                },
                body:
                    jsonEncode({'file': fileName, 'msg_id': msgId, 'key': key}),
              )
              .timeout(const Duration(seconds: 20));
          if (r.statusCode == 200) return null;
        } catch (_) {}
        await Future<void>.delayed(const Duration(seconds: 2));
      }
      // Post kanalda bor — bot uni baribir ro'yxatga oladi.
      return null;
    }

    // Bot chatidan kanalga ko'chirilishini kutamiz (odatda 1-2 s).
    //
    // Fayl allaqachon Telegram'da va izohida kalit bor — qolgan ishni
    // (kanalga ko'chirish, bazaga yozish) bot o'zi qiladi. Shu sabab
    // kutish tugasa yoki internet sekinlashsa ham fayl QAYTA
    // YUKLANMAYDI: yuklash muvaffaqiyatli hisoblanadi. Faqat bot chati
    // bot ko'chirib ulgurgunicha tozalanmaydi (aks holda nusxa
    // o'chib ketardi) — u keyinroq tozalanadi.
    var ready = false;
    for (final wait in const [1, 1, 2, 2, 3, 4, 5, 8]) {
      await Future<void>.delayed(Duration(seconds: wait));
      try {
        final r = await http.get(
          // Kalit ham shu yerda yoziladi (faqat o'z faylingizga).
          Uri.parse('$kApiBase/api/tg/claim'
              '?file=${Uri.encodeQueryComponent(fileName)}'
              '&key=${Uri.encodeQueryComponent(key)}'),
          headers: {'Authorization': 'Bearer $s'},
        ).timeout(const Duration(seconds: 15));
        if (r.statusCode == 200 &&
            (jsonDecode(r.body) as Map<String, dynamic>)['ready'] == true) {
          ready = true;
          break;
        }
      } catch (_) {
        // Tarmoq sekin — keyingi urinish.
      }
    }
    if (ready) {
      // Bot chatidagi yuborilgan nusxa endi keraksiz.
      _readPending = true;
      if (_holders.isEmpty) unawaited(_markReadIfPending());
    } else {
      Timer(const Duration(minutes: 3), () {
        _readPending = true;
        if (_holders.isEmpty) unawaited(_markReadIfPending());
      });
    }
    return null;
  }

  /// Eski nom (qism qo'shish ekrani shu bilan chaqiradi).
  Future<String?> uploadToChannel(
    String path,
    String fileName,
    String mime,
    void Function(int sent, int total) onProgress,
  ) =>
      uploadFile(path, fileName, mime, onProgress: onProgress);

  // ── FAYLNI KO'RSATISH (rasmlar va h.k.) ─────────────────────
  //
  // Rasmlar ham worker'dan EMAS, Telegram'dan olinadi: ekranda
  // bir vaqtda so'ralgan hamma fayllar BITTA `/api/tg/deliver`
  // so'rovi bilan bot chatiga keladi (`copyMessages`), ilova ularni
  // mahalliy Telegram manbasidan (`127.0.0.1/tg/...`) oladi va o'z
  // shifrlangan keshiga yozadi. So'ng bot chati tozalanadi.

  final Map<String, List<Completer<Uint8List?>>> _batch = {};
  Timer? _batchTimer;

  /// Faylning baytlari (Telegram'da bo'lmasa yoki olib bo'lmasa `null`).
  Future<Uint8List?> fetchBytes(String url) {
    final name = fileNameOf(url);
    if (name.isEmpty || !active || _port == 0 || _missing.contains(name)) {
      return Future.value(null);
    }
    final c = Completer<Uint8List?>();
    _batch.putIfAbsent(name, () => []).add(c);
    _batchTimer ??= Timer(const Duration(milliseconds: 150), _runBatch);
    return c.future;
  }

  Future<void> _runBatch() async {
    _batchTimer = null;
    final batch = Map.of(_batch);
    _batch.clear();
    if (batch.isEmpty) return;
    void finish(String name, Uint8List? v) {
      for (final c in batch[name] ?? const <Completer<Uint8List?>>[]) {
        if (!c.isCompleted) c.complete(v);
      }
    }

    final owner = Object();
    try {
      await _markReadIfPending();
      final s = AuthService.instance.sessionToken;
      if (s == null) throw 'kirilmagan';
      final names = batch.keys.toList();
      hold(owner, names.first);
      // Avval bot chatidan; faqat yo'qlarini bot kanaldan yuboradi.
      final got = await _findInChat(names);
      if (got == null) throw 'bot chati tekshirilmadi';
      got.addAll(await _deliver(names
          .where((n) => !got.contains(n) && !_recentlyDelivered(n))
          .toList()));
      for (final n in names) {
        if (!got.contains(n)) {
          _missing.add(n);
          finish(n, null);
        }
      }
      // Mahalliy manbadan — bir vaqtda 4 tadan.
      final todo = names.where(got.contains).toList();
      for (var i = 0; i < todo.length; i += 4) {
        await Future.wait(todo.skip(i).take(4).map((n) async {
          try {
            final r = await http
                .get(Uri.parse('http://127.0.0.1:$_port/tg/0/$n'))
                .timeout(const Duration(seconds: 60));
            finish(n, r.statusCode == 200 ? r.bodyBytes : null);
          } catch (_) {
            finish(n, null);
          }
        }));
      }
    } catch (_) {
      for (final n in batch.keys) {
        finish(n, null);
      }
    } finally {
      unhold(owner);
    }
  }

  /// Rust yadrosiga bot nomini beradi — videolar shu bot chatidan
  /// fayl NOMI bo'yicha topiladi (`fetch_doc` izohi).
  void _setBot(String bot) {
    final lib = _lib;
    if (lib == null || bot.isEmpty) return;
    final f = lib.lookupFunction<Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('rust_tg_set_bot');
    final b = bot.toNativeUtf8();
    try {
      f(b);
    } finally {
      malloc.free(b);
    }
  }

  // ── SAQLANGAN KIRISH BOSQICHI ───────────────────────────────
  //
  // TALAB: ilova yopilib qayta ochilsa ham kod (yoki parol) oynasi
  // o'zi ochilsin. Bosqich Rust yadrosida shifrlangan faylda turadi.

  /// `{"stage": "phone"|"code"|"password"|"done", "phone", "hint"}`.
  Map<String, dynamic> loginState() {
    final lib = _lib;
    if (lib == null) return const {'stage': 'phone'};
    return _json(_take(
        lib, lib.lookupFunction<_NoArgC, _NoArgC>('rust_tg_login_state')()));
  }

  /// "Raqamni o'zgartirish".
  void resetLogin() {
    final lib = _lib;
    if (lib == null) return;
    lib.lookupFunction<Void Function(), void Function()>(
        'rust_tg_login_reset')();
  }

  // ── BOT CHATINI TOZALASH ────────────────────────────────────
  //
  // TALAB (foydalanuvchi): "pleyerni tark etganda yoki internetni
  // o'chirishi bilan bot tarixni avtomatik o'chirsin" va "workerga,
  // tursoga iloji boricha kamroq so'rov".
  //
  // ── ENDI TOZALANMAYDI — FAQAT "O'QILGAN" (2026-09-25) ────────
  //
  // TALAB (foydalanuvchi): "bot tarixi tozalanmasin — fayllar
  // shifrlangan; kerakli fayl avval bot chatidan nomi bilan izlansin,
  // bor bo'lsa kanaldan qayta copy qilinmasin; faqat nusxalar
  // O'QILGAN deb belgilansin, Telegram'da botda 'N ta o'qilmagan'
  // chiqmasin".
  //
  // Shu sabab ilgari chat tozalanadigan hamma joyda endi chat
  // foydalanuvchining O'Z hisobi bilan o'qilgan deb belgilanadi
  // (`rust_tg_mark_read`, `messages.readHistory`) — worker ham, baza
  // ham ishtirok etmaydi. Nusxa chatda qoladi va keyingi safar
  // `rust_tg_find` uni topadi (bot qayta yubormaydi).

  final Map<Object, String> _holders = {};
  bool _readPending = false;
  Future<void>? _marking;
  Timer? _readTimer;
  StreamSubscription<List<ConnectivityResult>>? _connSub;

  /// [owner] shu nusxani ishlatyapti (pleyer ekrani, yuklab olish).
  void hold(Object owner, String url) {
    final name = fileNameOf(url);
    if (name.isEmpty) return;
    _holders[owner] = name;
    _readTimer?.cancel();
  }

  /// [owner] nusxani qo'yib yubordi.
  void unhold(Object owner) {
    if (_holders.remove(owner) == null) return;
    if (_holders.isNotEmpty) return;
    // Bir zum kutamiz: boshqa qismga o'tilayotgan bo'lsa yangi
    // nusxa kelib, darhol yana band bo'ladi.
    _readTimer?.cancel();
    _readTimer = Timer(const Duration(seconds: 2), () {
      if (_holders.isEmpty) {
        _readPending = true;
        unawaited(_markReadIfPending());
      }
    });
  }

  void _watchConnectivity() {
    if (_connSub != null) return;
    try {
      _connSub = Connectivity().onConnectivityChanged.listen((r) {
        final online =
            r.isNotEmpty && !r.every((e) => e == ConnectivityResult.none);
        if (!online) {
          // Internet yo'q — tozalash navbatda.
          //
          // Band qilganlar (pleyer, yuklab olish) BEKOR QILINMAYDI:
          // foydalanuvchi talabi — internet qaytgach video AYNI
          // joydan davom etsin. Ilgari bu yerda hamma band o'chirilar
          // edi va internet qaytishi bilan bot chati tozalanib,
          // pleyer ko'rayotgan nusxa ham yo'qolardi — video xatoga
          // chiqib boshidan boshlanardi. Chat pleyer yopilgach
          // tozalanadi (`unhold`).
          _readPending = true;
        } else {
          unawaited(_markReadIfPending());
        }
      });
    } catch (_) {}
  }

  /// Navbatdagi tozalashni bajaradi (bir vaqtda faqat bittasi).
  Future<void> _markReadIfPending() {
    final running = _marking;
    if (running != null) return running;
    if (!_readPending || !_authorized) return Future.value();
    final f = () async {
      try {
        final j = await Isolate.run(() {
          final lib = _openLib();
          return _json(_take(
              lib,
              lib.lookupFunction<_NoArgC, _NoArgC>(
                  'rust_tg_mark_read')()));
        });
        if (j['ok'] == true) _readPending = false;
      } catch (_) {
        // Tarmoq yo'q — navbatda qoladi.
      }
    }();
    _marking = f.whenComplete(() => _marking = null);
    return _marking!;
  }

  void _afterLogin() {
    _authorized = true;
    _missing.clear();
    notifyListeners();
    // Oldingi (uzilgan) sessiya davrida bot chatida qolgan nusxalar
    // endi yangi sessiya bilan o'chiriladi.
    _readPending = true;
    unawaited(_markReadIfPending());
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

  /// Android pleyeri uchun manba (`AruDataSource`): pleyer diskdagi
  /// shifrlangan bo'laklarni o'zi o'qiydi, yo'g'ini Telegram'dan
  /// olib diskka yozadi. [size] ma'lum bo'lsa (`epizod_db.size_*`)
  /// ochishda Telegram'ga hajm so'rovi ketmaydi.
  static Uri aruUri(String url, {int size = 0}) => Uri.parse(
      'aru://file/${fileNameOf(url)}${size > 0 ? '?size=$size' : ''}');

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
    // ── BITTA FAYL — BITTA TAYYORLASH ─────────────────────────
    //
    // TOPILGAN XATO (foydalanuvchi: "bitta video bot chatiga 3-4
    // marta copy qilindi"): pleyer, yuklab olish va qayta urinishlar
    // `prepare` ni bir vaqtda (yoki 20 soniyalik kutish tugab, eski
    // chaqiruv hali ishlayotgan paytda) chaqirardi — har biri
    // o'zicha botdan nusxa so'rardi. Endi bir nom uchun faqat BITTA
    // tayyorlash yuradi, qolganlar o'sha natijani kutadi (kutish
    // tugasa ham tayyorlash to'xtamaydi — keyingi chaqiruv unga
    // qo'shiladi).
    final name = fileNameOf(url);
    final running = _preparing[name];
    final f = running ??
        (_preparing[name] = _prepare(url).whenComplete(() {
          _preparing.remove(name);
        }));
    try {
      return await f.timeout(const Duration(seconds: 20));
    } catch (e) {
      debugPrint('Telegram: tayyorlanmadi: $e');
      return null;
    }
  }

  final Map<String, Future<String?>> _preparing = {};

  /// Bot shu fayllarni qachon yuborgan. Yaqinda (10 daqiqa) yuborilgan
  /// fayl chatda darhol ko'rinmasa ham QAYTA SO'RALMAYDI — chat yana
  /// bir necha marta qaraladi.
  final Map<String, DateTime> _deliveredAt = {};
  static const _redeliverAfter = Duration(minutes: 10);

  bool _recentlyDelivered(String name) {
    final at = _deliveredAt[name];
    return at != null && DateTime.now().difference(at) < _redeliverAfter;
  }

  /// Bu sessiyada kaliti serverdan yangilangan fayllar. Chatdagi
  /// nusxa topilganda kalit bir marta serverdan olinadi — fayl qayta
  /// yuklangan bo'lsa telefondagi eski kalit bilan ochilmasin.
  final Set<String> _keysFresh = {};

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

  /// Bu fayllar bot chatida BORmi (`rust_tg_find`) — kanaldan qayta
  /// nusxa so'rashdan OLDIN. Chatda bor-u kaliti telefonda yo'qlari
  /// uchun kalit serverdan nusxasiz olinadi (`keys_only`).
  ///
  /// `null` — chatni tekshirib bo'lmadi (tarmoq): bunda nusxa
  /// SO'RALMAYDI, aks holda har bir uzilish yangi nusxa bo'lardi.
  Future<Set<String>?> _findInChat(List<String> names) async {
    if (names.isEmpty) return {};
    Map<String, dynamic> j;
    try {
      final arg = jsonEncode(names);
      j = await Isolate.run(() {
        final lib = _openLib();
        final f = lib.lookupFunction<_StrArgC, _StrArgC>('rust_tg_find');
        final a = arg.toNativeUtf8();
        try {
          return _json(_take(lib, f(a)));
        } finally {
          malloc.free(a);
        }
      });
    } catch (_) {
      return null;
    }
    if (j['error'] != null) return null;
    final found = ((j['found'] as List?) ?? const []).whereType<String>().toSet();
    final noKey = {
      ...((j['no_key'] as List?) ?? const []).whereType<String>(),
      ...found.where((n) => !_keysFresh.contains(n)),
    }.toList();
    if (noKey.isNotEmpty) {
      final got = await _deliver(noKey, keysOnly: true);
      // Kaliti olinmagani (ruxsat yo'q) — chatdagi nusxa ishlatilmaydi.
      _keysFresh.addAll(got);
      found.removeWhere((n) => noKey.contains(n) && !got.contains(n));
    }
    return found;
  }

  /// `/api/tg/deliver` — bot fayllarni chatga yuboradi ([keysOnly] —
  /// faqat kalitlar, nusxasiz). Qaytadi: server bergan fayllar.
  Future<Set<String>> _deliver(List<String> names,
      {bool keysOnly = false}) async {
    final s = AuthService.instance.sessionToken;
    if (s == null || names.isEmpty) return {};
    final got = <String>{};
    for (var i = 0; i < names.length; i += 100) {
      final part = names.sublist(i, (i + 100).clamp(0, names.length));
      try {
        final r = await http
            .post(
              Uri.parse('$kApiBase/api/tg/deliver'),
              headers: {
                'Authorization': 'Bearer $s',
                'Content-Type': 'application/json',
              },
              body: jsonEncode({'files': part, 'keys_only': keysOnly}),
            )
            .timeout(const Duration(seconds: 25));
        if (r.statusCode == 200) {
          final j = jsonDecode(r.body) as Map<String, dynamic>;
          _applyKeys(j['keys']);
          final files = j['files'];
          if (files is List) got.addAll(files.whereType<String>());
        }
      } catch (_) {}
    }
    if (keysOnly) {
      _keysFresh.addAll(got);
    } else {
      final now = DateTime.now();
      for (final n in got) {
        _deliveredAt[n] = now;
        _keysFresh.add(n);
      }
    }
    // Bot yangi nusxa yubordi — chat o'qilgan deb belgilanadi.
    if (!keysOnly && got.isNotEmpty) {
      _readPending = true;
      unawaited(_markReadIfPending());
    }
    return got;
  }

  Future<String?> _prepare(String url) async {
    _syncStatus();
    // Bot nusxa yuborishidan OLDIN sessiya tirikligi tekshiriladi —
    // aks holda o'lik sessiyaga nusxa yuborilib, u o'chirilmay
    // qolardi (30 soniyada bir marta).
    await checkSession();
    if (!active) return null;
    unawaited(_markReadIfPending());
    final name = fileNameOf(url);
    if (name.isEmpty || _missing.contains(name)) return null;

    // Nusxa bot chatida hali turibdi — worker'ga so'rov YO'Q.
    final ready = _playUrl(name);
    if (ready.isNotEmpty) return ready;

    // 1) Avval bot chatidan — bor bo'lsa kanaldan qayta nusxa YO'Q.
    // Yaqinda yuborilgan bo'lsa chat bir necha marta qaraladi (nusxa
    // chatda bir-ikki soniyada paydo bo'ladi).
    for (var i = 0;; i++) {
      final found = await _findInChat([name]);
      // Chatni tekshirib bo'lmadi — nusxa so'ralmaydi.
      if (found == null) return null;
      if (found.contains(name)) return _foundUrl(name);
      if (!_recentlyDelivered(name) || i >= 3) break;
      await Future<void>.delayed(const Duration(milliseconds: 1500));
    }
    // Yaqinda yuborilgan-u chatda yo'q (foydalanuvchi o'chirgan
    // bo'lishi mumkin) — faqat 10 daqiqadan keyin qayta so'raladi.
    if (_recentlyDelivered(name)) return null;

    // 2) Chatda yo'q — bot kanaldan nusxa yuboradi.
    final s = AuthService.instance.sessionToken;
    if (s == null) return null;
    final r = await http.post(
      Uri.parse('$kApiBase/api/tg/deliver'),
      headers: {
        'Authorization': 'Bearer $s',
        'Content-Type': 'application/json',
      },
      body: jsonEncode({'file': name}),
    );
    if (r.statusCode == 404) {
      // Bu qism hali kanalga yuklanmagan — B2'dan ko'riladi.
      _missing.add(name);
      return null;
    }
    if (r.statusCode != 200) return null;
    final j = jsonDecode(r.body) as Map<String, dynamic>;
    _applyKeys(j['keys']);
    final msgId = (j['msg_id'] as num?)?.toInt() ?? 0;
    if (msgId <= 0) return null;
    _deliveredAt[name] = DateTime.now();
    _keysFresh.add(name);
    _readPending = true;
    unawaited(_markReadIfPending());
    _route(name, msgId);
    final u = _playUrl(name);
    return u.isEmpty ? null : u;
  }

  /// Chatda topilgan fayl manzili (`rust_tg_find` oldingi o'qish
  /// xatosi chetlatishini o'zi olib tashlaydi — yangi nusxa kerak emas).
  String? _foundUrl(String name) {
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
  }
}
