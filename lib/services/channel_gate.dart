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
//   3. Orqa fonda (ilova ochiq turganda, har 5 daqiqada) ro'yxat
//      (`GET /api/channels`, 15 daqiqada bir marta) olinadi va hali
//      qo'shilinmagan har bir kanalga foydalanuvchining O'Z Telegram
//      hisobi bilan qo'shiladi (ochiq kanal) yoki so'rov yuboriladi
//      (yopiq kanal) — `rust_tg_join_channel`.
//   4. Bajarilgani telefonda eslab qolinadi (`chan_done`) — bir
//      kanalga qayta-qayta urinilmaydi. Telegram FLOOD_WAIT bersa,
//      shu aylanish to'xtaydi va keyingisida davom etadi.
//
// Serverga faqat ruxsatning o'zi yoziladi (sozlama). Kim qo'shilgani/so'rov yuborgani
// bot orqali, Telegram hodisalari bilan sanaladi (`worker/src/channels.rs`).

import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;

import 'auth_service.dart';
import 'rust_bridge.dart';
import 'telegram_service.dart';

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

  /// Orqa fondagi urinishlar oralig'i (foydalanuvchi talabi: 1-5 daqiqa).
  static const Duration _every = Duration(minutes: 5);

  /// Ro'yxat serverdan shundan tez-tez so'ralmaydi (worker'ga kamroq so'rov).
  static const Duration _listTtl = Duration(minutes: 15);

  /// Ikki qo'shilish orasidagi tanaffus (Telegram cheklovini yoqmaslik uchun).
  static const Duration _gap = Duration(seconds: 4);

  int _uid = -1;
  bool _consent = false;
  final Set<String> _done = {};
  List<GateChannel>? _list;
  int _listAt = 0;
  Timer? _timer;
  bool _running = false;

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
    _first?.cancel();
    _first = null;
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

  /// Birinchi qo'shilishgacha kutish.
  ///
  /// TOPILGAN MUAMMO (foydalanuvchi: "bepul ko'rishda ilova juda sekin,
  /// qismlar sekin yuklanyapti"): ruxsat berilishi bilan (ya'ni AYNAN
  /// pleyer ochilib, video yuklana boshlagan paytda) ilova har bir
  /// kanalga ketma-ket qo'shila boshlardi — Telegram ulanishi videoning
  /// birinchi bo'laklari bilan talashardi. Endi video boshlanib olgach.
  static const Duration _firstDelay = Duration(seconds: 45);
  Timer? _first;

  void _arm() {
    _timer ??= Timer.periodic(_every, (_) => unawaited(_tick()));
    _first ??= Timer(_firstDelay, () {
      _first = null;
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

  /// Bitta aylanish: hali bajarilmagan kanallarga qo'shiladi.
  Future<void> _tick() async {
    if (_running || !consented) return;
    // Ilova fonda bo'lsa ham taymer ishlashi mumkin — bu zararsiz:
    // ro'yxat 15 daqiqada bir marta, qo'shilish faqat yangi kanalga.
    final tg = TelegramService.instance;
    if (!tg.authorized) return;
    _running = true;
    try {
      await refresh();
      final todo = (_list ?? const <GateChannel>[])
          .where((c) => c.url.isNotEmpty && !_done.contains(c.doneKey))
          .toList();
      for (final c in todo) {
        if (!consented) break;
        final j = await tgCall('rust_tg_join_channel', arg: '${c.kind}:${c.url}');
        if (j['ok'] == true) {
          _done.add(c.doneKey);
          _saveDone();
        } else if (j['wait'] != null || '${j['error']}'.contains('FLOOD')) {
          // Telegram "kuting" dedi — shu aylanish tugaydi.
          break;
        } else if (_permanent('${j['error']}')) {
          // Havola yaroqsiz / kanal yo'q — admin tuzatmaguncha urinilmaydi
          // (manzil o'zgarsa kalit ham o'zgaradi).
          _done.add(c.doneKey);
          _saveDone();
        }
        await Future<void>.delayed(_gap);
      }
    } finally {
      _running = false;
    }
  }

  static bool _permanent(String e) =>
      e.contains('INVITE_HASH_EXPIRED') ||
      e.contains('INVITE_HASH_INVALID') ||
      e.contains('USERNAME_NOT_OCCUPIED') ||
      e.contains('USERNAME_INVALID') ||
      e.contains('CHANNELS_TOO_MUCH') ||
      e.contains('noto\'g\'ri') ||
      e.contains('topilmadi');

  void _saveDone() {
    try {
      RustCore.instance
          .saveListCache(_doneKey, [for (final k in _done) {'k': k}]);
    } catch (_) {}
  }
}
