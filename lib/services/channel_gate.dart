// lib/services/channel_gate.dart — MAJBURIY OBUNA KANALLARI.
//
// ═══════════════════════════════════════════════════════════════
//  TALAB (foydalanuvchi)
// ═══════════════════════════════════════════════════════════════
//
// "Foydalanuvchi bepul animeni ko'rishi uchun ilova undan kanallarga
//  obuna bo'lish uchun ruxsat so'raydi. Ruxsat berishi bilan pleyer
//  sahifasi ochilsin, ilova esa orqada har 5 daqiqada, agar kanal
//  mavjud bo'lsa, obuna bo'ladi yoki so'rov yuboradi."
//
// ── QANDAY ISHLAYDI ─────────────────────────────────────────
//
//   1. Obunasi yo'q odam BEPUL bo'limni ochadi -> pleyer o'rnida
//      ruxsat oynasi (`video_player_screen.dart` -> `_ChannelConsentScreen`).
//      Admin birorta ham kanal qo'shmagan bo'lsa oyna chiqmaydi.
//   2. "Ruxsat berish" -> ruxsat hisobga yoziladi va bazada saqlanadi
//      (`users_db.chan_consent`, `SyncQueue` orqali — boshqa
//      sozlamalar kabi), pleyer DARHOL ochiladi.
//   3. Orqa fonda ro'yxat (`GET /api/channels`, 15 daqiqada bir
//      marta) olinadi va hali qo'shilinmagan kanallarga
//      foydalanuvchining O'Z Telegram hisobi bilan BITTADAN, har
//      biri orasida 5-10 SONIYA tanaffus bilan qo'shiladi (ochiq
//      kanal) yoki so'rov yuboriladi (yopiq kanal) — `rust_tg_join_channel`.
//      Hammasi bajarilgach yangi kanal 5 daqiqada bir tekshiriladi.
//   4. Bajarilgani telefonda eslab qolinadi (`chan_done`) — bir
//      kanalga qayta-qayta urinilmaydi.
//
// ── PLEYER OCHIQ BO'LSA — OBUNA YO'Q ─────────────────────────
//
// TALAB (foydalanuvchi): "pleyerga kirganda ilova obuna bo'lmasdan
// faqat video va ma'lumotlarga e'tibor qaratsin — shu sabab sekin
// yuklanyapti". Video ekrani ochiq ekan (`VideoGate.busy`) birorta
// ham qo'shilish yoki ro'yxat so'rovi qilinmaydi; ekran yopilgach
// navbat o'z joyidan davom etadi.
//
// ── TELEGRAM LIMITI — YARIM SOAT KUTISH ──────────────────────
//
// Telegram "kuting" (FLOOD_WAIT) yoki "kanallar juda ko'p"
// (CHANNELS_TOO_MUCH) desa — 30 daqiqa (Telegram ko'proq desa,
// o'shancha) hech narsa qilinmaydi. Muddat telefonda saqlanadi
// (`chan_pause`): ilova qayta ochilsa ham kutish buzilmaydi.
//
// Serverga faqat ruxsatning o'zi yoziladi (sozlama). Kim qo'shilgani/so'rov yuborgani
// bot orqali, Telegram hodisalari bilan sanaladi (`worker/src/channels.rs`).

import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;

import 'auth_service.dart';
import 'rust_bridge.dart';
import 'telegram_service.dart';
import 'video_gate.dart';

/// Ro'yxatdagi bitta kanal yoki havola.
class GateChannel {
  final int id;

  /// `public` | `private`.
  final String kind;
  final String title;
  final String url;

  const GateChannel(this.id, this.kind, this.title, this.url);

  /// Bajarilganini eslab qolish kaliti: manzil o'zgarsa (admin kanalni
  /// o'chirib qayta qo'shsa) qayta uriniladi.
  String get doneKey => '$kind|$url';

  factory GateChannel.fromJson(Map<String, dynamic> j) => GateChannel(
        (j['id'] as num?)?.toInt() ?? 0,
        '${j['kind'] ?? ''}',
        '${j['title'] ?? ''}',
        '${j['url'] ?? ''}',
      );
}

class ChannelGate extends ChangeNotifier {
  ChannelGate._();
  static final ChannelGate instance = ChannelGate._();

  static const _consentKey = 'chan_consent';
  static const _doneKey = 'chan_done';
  static const _listKey = 'chan_list';
  static const _pauseKey = 'chan_pause';

  /// Hamma kanal bajarilgach yangisi shu oraliqda tekshiriladi.
  static const Duration _every = Duration(minutes: 5);

  /// Telegram limitiga yetilganda kutish (foydalanuvchi talabi).
  static const Duration _limitPause = Duration(minutes: 30);

  /// Pleyer ochiq paytda shu oraliqda (tarmoqsiz) qaraladi: yopildimi.
  static const Duration _busyPoll = Duration(seconds: 10);

  /// Ro'yxat serverdan shundan tez-tez so'ralmaydi (worker'ga kamroq so'rov).
  static const Duration _listTtl = Duration(minutes: 15);

  /// Ikki qo'shilish orasidagi tanaffus: 5-10 soniya (foydalanuvchi talabi).
  static Duration _gap() =>
      Duration(milliseconds: 5000 + Random().nextInt(5001));

  int _uid = -1;
  bool _consent = false;
  final Set<String> _done = {};
  List<GateChannel>? _list;
  int _listAt = 0;
  Timer? _timer;
  bool _running = false;

  /// Telegram limiti: shu vaqtgacha (ms) qo'shilish yo'q.
  int _pauseUntil = 0;

  /// Shu aylanishda vaqtincha xato bergan kanallar (internet va h.k.):
  /// navbat ularga tiqilib qolmasin — keyingi 5 daqiqalik aylanishda.
  final Set<String> _skip = {};

  /// Eski (telefondagi) ruxsat bazaga ko'chirilmoqda (`_sync`).
  bool _migrating = false;

  /// Hisob almashsa (papka boshqa) — qaytadan o'qiladi.
  void _sync() {
    final uid = AuthService.instance.user?.id ?? 0;
    if (uid == _uid) return;
    _uid = uid;
    _consent = false;
    _done.clear();
    _list = null;
    _listAt = 0;
    _pauseUntil = 0;
    _skip.clear();
    try {
      final u = AuthService.instance.user;
      final c = RustCore.instance.getCachedList(_consentKey);
      final local = c != null && c.isNotEmpty;
      if (u == null) {
        // Hisobsiz (mehmon) — ruxsat faqat telefonda.
        _consent = local;
      } else {
        _consent = u.chanConsent;
        // ── ESKI ILOVADAN QOLGAN RUXSAT ─────────────────────
        // Ilgari ruxsat faqat telefonda (`chan_consent`) turardi.
        // Bir marta bazaga ko'chiriladi va telefondagisi o'chiriladi.
        // `_sync` ekran chizilayotganda ham chaqiriladi — hisob
        // o'zgarishi keyingi lahzaga qoldiriladi.
        if (local) {
          RustCore.instance
              .saveListCache(_consentKey, const <Map<String, dynamic>>[]);
          if (!_consent) {
            _consent = true;
            _migrating = true;
            scheduleMicrotask(() {
              _migrating = false;
              AuthService.instance.updateSettings(chanConsent: true);
            });
          }
        }
      }
      for (final e in RustCore.instance.getCachedList(_doneKey) ?? const []) {
        final k = e['k'];
        if (k is String) _done.add(k);
      }
      final p = RustCore.instance.getCachedList(_pauseKey);
      if (p != null && p.isNotEmpty) {
        _pauseUntil = (p.first['until'] as num?)?.toInt() ?? 0;
      }
      final l = RustCore.instance.getCachedList(_listKey);
      if (l != null) {
        _list = l.map(GateChannel.fromJson).toList();
      }
    } catch (_) {}
  }

  bool get consented {
    _sync();
    // Ruxsat boshqa qurilmada (yoki serverdan yangilanib) o'zgargan
    // bo'lishi mumkin — haqiqiy manba hisobning o'zi.
    final u = AuthService.instance.user;
    if (u != null &&
        !_migrating &&
        u.chanConsent != _consent &&
        u.id == _uid) {
      _consent = u.chanConsent;
      if (!_consent) {
        _timer?.cancel();
        _timer = null;
      }
    }
    return _consent;
  }

  /// Hozir talab qilinayotgan kanallar (`null` — hali noma'lum).
  List<GateChannel>? get channels {
    _sync();
    return _list;
  }

  /// Ruxsat so'rash kerakmi: ruxsat yo'q va ro'yxatda (yoki hali
  /// noma'lum) birorta kanal bor.
  bool get needsConsent {
    if (consented) return false;
    final l = _list;
    return l == null || l.isNotEmpty;
  }

  /// Hisob almashdi (`account_data.dart`) — keyingi murojaatda yangi
  /// papkadan o'qiladi; ruxsati bo'lsa orqa fon davom etadi.
  void reset() {
    _uid = -1;
    notifyListeners();
    start();
  }

  /// Ilova ochilganda: ruxsat avval berilgan bo'lsa orqa fon ishga tushadi.
  void start() {
    if (consented) _arm();
  }

  /// Foydalanuvchi "Ruxsat berish"ni bosdi.
  Future<void> grant() async {
    _sync();
    _consent = true;
    _saveConsent(true);
    notifyListeners();
    _arm();
  }

  /// Ruxsat o'chirildi (Sozlamalar): orqa fondagi qo'shilish to'xtaydi,
  /// bepul bo'lim ochilganda ruxsat yana so'raladi. Allaqachon qo'shilgan
  /// kanallardan chiqilmaydi — buni foydalanuvchi Telegram'da o'zi qiladi.
  void revoke() {
    _sync();
    _consent = false;
    _timer?.cancel();
    _timer = null;
    _saveConsent(false);
    notifyListeners();
  }

  /// Hisob bo'lsa — bazaga (`SyncQueue`), bo'lmasa telefonga.
  void _saveConsent(bool on) {
    if (AuthService.instance.user != null) {
      AuthService.instance.updateSettings(chanConsent: on);
      return;
    }
    try {
      RustCore.instance.saveListCache(_consentKey, [
        if (on) {'at': DateTime.now().millisecondsSinceEpoch}
      ]);
    } catch (_) {}
  }

  /// Navbatdagi aylanish `after` dan keyin (bitta taymer).
  ///
  /// Ilgari ruxsat berilgach 45 soniya kutilib, keyin har 5 daqiqada
  /// hamma kanalga ketma-ket qo'shilinardi — bu pleyer ochiq paytga
  /// to'g'ri kelib videoni sekinlashtirardi. Endi pleyer ochiq ekan
  /// `_tick` hech narsa qilmaydi (`VideoGate.busy`).
  void _arm([Duration after = const Duration(seconds: 5)]) {
    _timer?.cancel();
    _timer = Timer(after, () {
      _timer = null;
      unawaited(_tick());
    });
  }

  /// Ro'yxatni serverdan oladi (ruxsat oynasi ham chaqiradi).
  Future<void> refresh({bool force = false}) async {
    _sync();
    final now = DateTime.now().millisecondsSinceEpoch;
    if (!force && _list != null && now - _listAt < _listTtl.inMilliseconds) {
      return;
    }
    final token = AuthService.instance.sessionToken;
    if (token == null || token.isEmpty) return;
    try {
      final r = await http
          .get(Uri.parse('$kApiBase/api/channels'),
              headers: {'Authorization': 'Bearer $token'})
          .timeout(const Duration(seconds: 15));
      if (r.statusCode != 200) return;
      final j = jsonDecode(r.body) as Map<String, dynamic>;
      final items = (j['items'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .toList();
      _list = items.map(GateChannel.fromJson).toList();
      _listAt = now;
      try {
        RustCore.instance.saveListCache(_listKey, items);
      } catch (_) {}
      notifyListeners();
    } catch (_) {
      // Internet yo'q — keyingi aylanishda.
    }
  }

  /// Bitta qadam: navbatdagi BITTA kanalga qo'shiladi va keyingi
  /// qadam rejalashtiriladi.
  Future<void> _tick() async {
    if (_running || !consented) return;
    // Pleyer ochiq — tarmoq faqat videoniki. Tarmoqsiz kutib turiladi.
    if (VideoGate.busy) return _arm(_busyPoll);
    final now = DateTime.now().millisecondsSinceEpoch;
    if (now < _pauseUntil) {
      return _arm(Duration(milliseconds: _pauseUntil - now));
    }
    final tg = TelegramService.instance;
    if (!tg.authorized) return _arm(_every);
    _running = true;
    var next = _every;
    try {
      await refresh();
      if (VideoGate.busy) {
        next = _busyPoll;
        return;
      }
      final todo = (_list ?? const <GateChannel>[])
          .where((c) =>
              c.url.isNotEmpty &&
              !_done.contains(c.doneKey) &&
              !_skip.contains(c.doneKey))
          .toList();
      if (todo.isEmpty) {
        // Hammasi bajarildi (yoki qolganlari vaqtincha xato berdi) —
        // 5 daqiqadan keyin yangi kanal bormi qaraladi.
        _skip.clear();
        return;
      }
      final c = todo.first;
      final j = await tgCall('rust_tg_join_channel', arg: '${c.kind}:${c.url}');
      final err = '${j['error'] ?? ''}';
      if (j['ok'] == true) {
        _done.add(c.doneKey);
        _saveDone();
      } else if (_limit(err) || j['wait'] != null) {
        // Telegram limiti — yarim soat (Telegram ko'proq desa, o'shancha).
        var wait = _limitPause;
        final secs = _waitSecs(err) ?? (j['wait'] as num?)?.toInt();
        if (secs != null && secs > wait.inSeconds) {
          wait = Duration(seconds: secs);
        }
        _setPause(DateTime.now().add(wait).millisecondsSinceEpoch);
        next = wait;
        return;
      } else if (_permanent(err)) {
        // Havola yaroqsiz / kanal yo'q — admin tuzatmaguncha urinilmaydi
        // (manzil o'zgarsa kalit ham o'zgaradi).
        _done.add(c.doneKey);
        _saveDone();
      } else {
        _skip.add(c.doneKey);
      }
      next = _gap();
    } finally {
      _running = false;
      if (consented) _arm(next);
    }
  }

  /// Telegram "ko'p qo'shilding, kut" yoki "kanallaring juda ko'p".
  static bool _limit(String e) =>
      e.contains('FLOOD') || e.contains('CHANNELS_TOO_MUCH');

  /// `FLOOD_WAIT ... (value: 300)` -> 300.
  static int? _waitSecs(String e) {
    final m = RegExp(r'value:\s*(\d+)').firstMatch(e) ??
        RegExp(r'FLOOD_(?:PREMIUM_)?WAIT_(\d+)').firstMatch(e);
    return m == null ? null : int.tryParse(m.group(1)!);
  }

  void _setPause(int until) {
    _pauseUntil = until;
    try {
      RustCore.instance.saveListCache(_pauseKey, [
        {'until': until}
      ]);
    } catch (_) {}
  }

  static bool _permanent(String e) =>
      e.contains('INVITE_HASH_EXPIRED') ||
      e.contains('INVITE_HASH_INVALID') ||
      e.contains('USERNAME_NOT_OCCUPIED') ||
      e.contains('USERNAME_INVALID') ||
      e.contains('noto\'g\'ri') ||
      e.contains('topilmadi');

  void _saveDone() {
    try {
      RustCore.instance
          .saveListCache(_doneKey, [for (final k in _done) {'k': k}]);
    } catch (_) {}
  }
}
