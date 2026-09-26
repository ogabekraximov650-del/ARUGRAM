// lib/services/tg_media.dart — Telegram stikerlari, maxsus emoji va
// GIF'lar (yozish paneli va xabarlarda ko'rsatish uchun).
//
// TALAB (foydalanuvchi): "izoh va support chatga Telegram emoji, GIF
// va stikerlarni ulab ber — Telegram'dagi pastki panel qanday ishlasa
// xuddi shunday; premium emoji'ni faqat premium'i bor odam yubora
// olsin".
//
// Hammasi foydalanuvchining O'Z Telegram hisobi bilan olinadi (Rust:
// `rust_tg_sticker_sets`, `rust_tg_media_file`, ...):
//   * stiker xabarda `stk_<to'plam>_<hash>_<hujjat>` havolasi bo'lib
//     turadi — ko'ruvchi uni o'zi oladi, bazaga fayl yozilmaydi;
//   * maxsus emoji matnda `[ce:<hujjat>:<emoji>]` belgisi bilan;
//   * GIF — kanalga joylangan oddiy fayl (`cmt_`/`chat_` nomi bilan),
//     xuddi video kabi olinadi.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';

import 'telegram_service.dart';

/// Telegram hujjati (stiker, maxsus emoji yoki GIF).
class TgDoc {
  final String id;

  /// `webp`, `tgs`, `webm`, `mp4` yoki `image`.
  final String kind;
  final String emoji;
  final String setId;
  final String setHash;
  final int w;
  final int h;
  final bool thumb;
  final bool custom;
  final bool free;

  const TgDoc({
    required this.id,
    required this.kind,
    this.emoji = '',
    this.setId = '0',
    this.setHash = '0',
    this.w = 0,
    this.h = 0,
    this.thumb = false,
    this.custom = false,
    this.free = true,
  });

  factory TgDoc.fromJson(Map<String, dynamic> j) => TgDoc(
        id: '${j['id'] ?? ''}',
        kind: '${j['kind'] ?? ''}',
        emoji: '${j['emoji'] ?? ''}',
        setId: '${j['set_id'] ?? '0'}',
        setHash: '${j['set_hash'] ?? '0'}',
        w: (j['w'] as num?)?.toInt() ?? 0,
        h: (j['h'] as num?)?.toInt() ?? 0,
        thumb: j['thumb'] == true,
        custom: j['custom'] == true,
        free: j['free'] != false,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'kind': kind,
        'emoji': emoji,
        'set_id': setId,
        'set_hash': setHash,
        'w': w,
        'h': h,
        'thumb': thumb,
        'custom': custom,
        'free': free,
      };

  /// Xabardagi stiker havolasi (`stk_<to'plam>_<hash>_<hujjat>`, u64 hex).
  String get ref => 'stk_${_hex(setId)}_${_hex(setHash)}_${_hex(id)}';

  /// Animatsiyalimi (Lottie yoki video).
  bool get animated => kind == 'tgs' || kind == 'webm' || kind == 'mp4';
}

/// Stiker yoki emoji to'plami.
class TgSet {
  final String id;
  final String hash;
  final String title;
  final int count;
  final String? thumbDoc;

  const TgSet(this.id, this.hash, this.title, this.count, this.thumbDoc);

  factory TgSet.fromJson(Map<String, dynamic> j) => TgSet(
        '${j['id']}',
        '${j['hash']}',
        '${j['title'] ?? ''}',
        (j['count'] as num?)?.toInt() ?? 0,
        j['thumb_doc'] as String?,
      );
}

String _hex(String dec) {
  final v = BigInt.tryParse(dec) ?? BigInt.zero;
  return v.toUnsigned(64).toRadixString(16);
}

String _dec(String hex) {
  final v = BigInt.tryParse(hex, radix: 16) ?? BigInt.zero;
  return v.toSigned(64).toString();
}

List<TgDoc> _docs(Object? v) => ((v as List?) ?? const [])
    .whereType<Map>()
    .map((e) => TgDoc.fromJson(e.cast<String, dynamic>()))
    .toList();

/// Matndagi maxsus emoji belgisi: `[ce:<hujjat id>:<oddiy emoji>]`.
final customEmojiToken = RegExp(r'\[ce:(-?\d{1,20}):([^\]]{1,16})\]');

class TgMedia {
  TgMedia._();
  static final instance = TgMedia._();

  bool get ready => TelegramService.instance.authorized;

  // ── PREMIUM ──────────────────────────────────────────────────
  bool? _premium;
  Future<bool> premium() async {
    final cached = _premium;
    if (cached != null) return cached;
    final j = await tgCall('rust_tg_premium');
    if (j['error'] != null) return false;
    return _premium = j['premium'] == true;
  }

  // ── STIKERLAR ────────────────────────────────────────────────
  Future<({List<TgSet> sets, List<TgDoc> recent, List<TgDoc> faved})>?
      _stickers;

  Future<({List<TgSet> sets, List<TgDoc> recent, List<TgDoc> faved})>
      stickers({bool refresh = false}) {
    if (refresh) _stickers = null;
    return _stickers ??= () async {
      final j = await tgCall('rust_tg_sticker_sets', intArg: 0);
      if (j['error'] != null) {
        _stickers = null;
        return (sets: <TgSet>[], recent: <TgDoc>[], faved: <TgDoc>[]);
      }
      return (
        sets: ((j['sets'] as List?) ?? const [])
            .whereType<Map>()
            .map((e) => TgSet.fromJson(e.cast<String, dynamic>()))
            .toList(),
        recent: _docs(j['recent']),
        faved: _docs(j['faved']),
      );
    }();
  }

  Future<List<TgSet>>? _emojiSets;
  Future<List<TgSet>> emojiSets() => _emojiSets ??= () async {
        final j = await tgCall('rust_tg_sticker_sets', intArg: 1);
        if (j['error'] != null) {
          _emojiSets = null;
          return <TgSet>[];
        }
        return ((j['sets'] as List?) ?? const [])
            .whereType<Map>()
            .map((e) => TgSet.fromJson(e.cast<String, dynamic>()))
            .toList();
      }();

  final Map<String, Future<List<TgDoc>>> _sets = {};

  /// To'plam ichidagi stikerlar.
  Future<List<TgDoc>> setDocs(String id, String hash) =>
      _sets[id] ??= () async {
        final j = await tgCall('rust_tg_sticker_set',
            arg: jsonEncode({'id': id, 'hash': hash}));
        if (j['error'] != null) {
          _sets.remove(id);
          return <TgDoc>[];
        }
        return _docs(j['docs']);
      }();

  /// Xabardagi stiker havolasidan hujjat.
  Future<TgDoc?> stickerByRef(String ref) async {
    final p = ref.split('_');
    if (p.length != 4 || p[0] != 'stk') return null;
    final docs = await setDocs(_dec(p[1]), _dec(p[2]));
    final id = _dec(p[3]);
    for (final d in docs) {
      if (d.id == id) return d;
    }
    return null;
  }

  // ── MAXSUS EMOJI ─────────────────────────────────────────────
  final Map<String, Completer<TgDoc?>> _emoji = {};
  final Set<String> _emojiQueue = {};
  Timer? _emojiTimer;

  /// Maxsus emoji hujjati ID bo'yicha. Ekrandagi hamma so'rovlar
  /// BITTA `getCustomEmojiDocuments` bilan olinadi.
  Future<TgDoc?> customEmoji(String id) {
    final c = _emoji[id];
    if (c != null) return c.future;
    final n = _emoji[id] = Completer<TgDoc?>();
    _emojiQueue.add(id);
    _emojiTimer ??= Timer(const Duration(milliseconds: 60), _flushEmoji);
    return n.future;
  }

  /// Panelda ko'rilgan maxsus emoji — qaytadan so'ralmasin.
  void rememberEmoji(TgDoc d) {
    final c = _emoji[d.id];
    if (c == null) {
      _emoji[d.id] = Completer<TgDoc?>()..complete(d);
    } else if (!c.isCompleted) {
      c.complete(d);
    }
  }

  Future<void> _flushEmoji() async {
    _emojiTimer = null;
    final ids = _emojiQueue.toList();
    _emojiQueue.clear();
    for (var i = 0; i < ids.length; i += 100) {
      final part = ids.sublist(i, (i + 100).clamp(0, ids.length));
      final j =
          await tgCall('rust_tg_custom_emoji', arg: jsonEncode(part));
      final got = {for (final d in _docs(j['docs'])) d.id: d};
      for (final id in part) {
        final c = _emoji[id];
        if (c == null || c.isCompleted) continue;
        final d = got[id];
        if (d == null && j['error'] != null) _emoji.remove(id);
        c.complete(d);
      }
    }
  }

  // ── GIF ──────────────────────────────────────────────────────
  Future<List<TgDoc>> savedGifs() async {
    final j = await tgCall('rust_tg_saved_gifs');
    return _docs(j['docs']);
  }

  Future<({List<TgDoc> docs, String next})> searchGifs(String q,
      {String offset = ''}) async {
    final j = await tgCall('rust_tg_gif_search',
        arg: jsonEncode({'q': q, 'offset': offset}));
    return (docs: _docs(j['docs']), next: '${j['next'] ?? ''}');
  }

  // ── FAYLLAR ──────────────────────────────────────────────────
  final Map<String, Future<String?>> _files = {};
  int _running = 0;
  final List<Completer<void>> _waiting = [];

  /// Hujjat (yoki kichik rasmi) diskdagi yo'li. Bir vaqtda eng ko'pi
  /// 4 ta yuklanadi — panel ochilganda 100 ta stiker birdan
  /// so'ralmasin.
  Future<String?> file(TgDoc d, {bool thumb = false}) async {
    final key = '${d.id}${thumb ? 't' : ''}';
    final p = await _file(d, key, thumb);
    // Xotira oynasida kesh tozalangan bo'lsa — qayta yuklanadi.
    if (p != null && !File(p).existsSync()) {
      _files.remove(key);
      return _file(d, key, thumb);
    }
    return p;
  }

  Future<String?> _file(TgDoc d, String key, bool thumb) {
    return _files[key] ??= () async {
      while (_running >= 4) {
        final c = Completer<void>();
        _waiting.add(c);
        await c.future;
      }
      _running++;
      try {
        final j = await tgCall('rust_tg_media_file',
            arg: jsonEncode({'id': d.id, 'thumb': thumb}));
        final p = j['path'] as String?;
        if (p == null) _files.remove(key);
        return p;
      } finally {
        _running--;
        if (_waiting.isNotEmpty) _waiting.removeAt(0).complete();
      }
    }();
  }

  // ── YAQINDA ISHLATILGANLAR (telefonda) ───────────────────────
  static const _recentMax = 40;
  List<String>? _recentEmoji;
  List<TgDoc>? _recentCustom;

  Future<File> _recentFile() async {
    final dir = await getApplicationSupportDirectory();
    return File('${dir.path}/tg_recent_emoji.json');
  }

  Future<void> _loadRecent() async {
    if (_recentEmoji != null) return;
    _recentEmoji = [];
    _recentCustom = [];
    try {
      final j = jsonDecode(await (await _recentFile()).readAsString())
          as Map<String, dynamic>;
      _recentEmoji = ((j['emoji'] as List?) ?? const []).whereType<String>().toList();
      _recentCustom = _docs(j['custom']);
    } catch (_) {}
  }

  Future<List<String>> recentEmoji() async {
    await _loadRecent();
    return List.of(_recentEmoji!);
  }

  Future<List<TgDoc>> recentCustom() async {
    await _loadRecent();
    return List.of(_recentCustom!);
  }

  Future<void> noteEmoji(String e) async {
    await _loadRecent();
    _recentEmoji!
      ..remove(e)
      ..insert(0, e);
    if (_recentEmoji!.length > _recentMax) _recentEmoji!.removeLast();
    await _saveRecent();
  }

  Future<void> noteCustom(TgDoc d) async {
    await _loadRecent();
    _recentCustom!
      ..removeWhere((x) => x.id == d.id)
      ..insert(0, d);
    if (_recentCustom!.length > _recentMax) _recentCustom!.removeLast();
    await _saveRecent();
  }

  Future<void> _saveRecent() async {
    try {
      await (await _recentFile()).writeAsString(jsonEncode({
        'emoji': _recentEmoji,
        'custom': _recentCustom!.map((d) => d.toJson()).toList(),
      }));
    } catch (_) {}
  }
}
