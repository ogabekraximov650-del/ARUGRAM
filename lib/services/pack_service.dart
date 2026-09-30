// lib/services/pack_service.dart — EMOJI, GIF VA STIKER TO'PLAMLARI.
//
// ═══════════════════════════════════════════════════════════════
//  TALAB
// ═══════════════════════════════════════════════════════════════
//
// Foydalanuvchi: "Telegramdagi premium emoji, GIF va stikerlarni
// ulaganda kuchsiz telefonlar ko'tara olmadi — ilova o'zining tizimini
// yasasin; foydalanuvchilar o'zlari yasaganini yuklasin".
//
// ═══════════════════════════════════════════════════════════════
//  QANDAY ISHLAYDI (batafsil — `worker/src/packs.rs`)
// ═══════════════════════════════════════════════════════════════
//
//   * TO'PLAM = kanaldagi BITTA shifrlangan fayl (`.arp`). Boshida
//     sarlavha (ro'yxat), keyin hamma kichik rasm (thumb), keyin
//     elementlarning o'zi. Fayl 1 GB gacha bo'lishi mumkin, shu sabab
//     hech qachon butunlay yuklanmaydi: faqat kerakli BO'LAK
//     (`Range`) mahalliy Telegram manbasidan olinadi.
//   * TELEFONGA BOSIM TUSHMASLIGI:
//       - to'plam oynasida faqat kichik STATIK rasmlar (~5 KB);
//       - animatsiya faqat kerak bo'lganda va bir vaqtda cheklangan
//         sondagina (`AnimSlots`, telefon kuchiga qarab);
//       - hamma narsa diskda shifrlab keshlanadi (hajm CHEKLANMAGAN —
//         foydalanuvchi talabi), bir marta olinadi;
//       - bo'laklar bir vaqtda 4 tadan ortiq olinmaydi.
//   * YOZUV: hamma yozuv `SyncQueue` orqali (Turso pul turadi).
//     Element qo'shish: fayl shifrlab bot chatiga yuklanadi (`pki_...`),
//     `add` amali yuboriladi -> admin ko'radi -> Actions to'plamga
//     qo'shadi. Rad etilsa sabab shu yerda ko'rinadi.
//   * Xabarda stiker/GIF — `pk_<to'plam>_<element>` havolasi; matn ichidagi
//     emoji — `[pe:<to'plam>:<element>:<oddiy emoji>]`.

import 'dart:async';
import 'dart:collection';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:characters/characters.dart';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;

import 'auth_service.dart';
import 'disk_cache.dart';
import 'rust_bridge.dart';
import 'sync_queue.dart';
import 'telegram_service.dart';

/// To'plam turlari.
class PackKind {
  const PackKind._();
  static const String sticker = 'sticker';
  static const String emoji = 'emoji';
  static const String gif = 'gif';
  static const List<String> all = [sticker, emoji, gif];

  static String plural(String k) => switch (k) {
        emoji => 'Emojilar',
        gif => 'GIFlar',
        _ => 'Stikerlar',
      };

  static String single(String k) => switch (k) {
        emoji => 'Emoji',
        gif => 'GIF',
        _ => 'Stiker',
      };
}

/// Chegaralar (worker va Actions bilan bir xil).
const int kPackItemMaxBytes = 5 * 1024 * 1024;
const int kPackMaxBytes = 1024 * 1024 * 1024;
const int kPackTitleMax = 40;

/// Bitta foydalanuvchining eng ko'p to'plami va ko'rib chiqilayotgan rasmi
/// (worker: `MAX_PACKS_PER_USER`, `MAX_PENDING_PER_USER`).
const int kPackMaxPerUser = 30;
const int kPackMaxWaiting = 100;

int _i(Object? v) => v is num ? v.toInt() : int.tryParse('$v') ?? 0;

class PackInfo {
  final int id;
  final String kind;
  final String title;

  /// Joriy fayl nomi (`pk_<id>_<versiya>.arp`); bo'sh — hali element yo'q.
  final String file;
  final int version;
  final int items;
  final int bytes;
  final int ownerId;
  final String ownerName;
  final bool sub;

  const PackInfo({
    required this.id,
    required this.kind,
    required this.title,
    this.file = '',
    this.version = 0,
    this.items = 0,
    this.bytes = 0,
    this.ownerId = 0,
    this.ownerName = '',
    this.sub = false,
  });

  factory PackInfo.fromJson(Map<String, dynamic> j) => PackInfo(
        id: _i(j['id']),
        kind: '${j['kind'] ?? PackKind.sticker}',
        title: '${j['title'] ?? ''}',
        file: '${j['file'] ?? ''}',
        version: _i(j['version']),
        items: _i(j['items']),
        bytes: _i(j['bytes']),
        ownerId: _i(j['owner_id']),
        ownerName: '${j['owner_name'] ?? ''}',
        sub: j['sub'] == true,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'kind': kind,
        'title': title,
        'file': file,
        'version': version,
        'items': items,
        'bytes': bytes,
        'owner_id': ownerId,
        'owner_name': ownerName,
        'sub': sub,
      };

  PackInfo copyWith({bool? sub}) => PackInfo(
        id: id,
        kind: kind,
        title: title,
        file: file,
        version: version,
        items: items,
        bytes: bytes,
        ownerId: ownerId,
        ownerName: ownerName,
        sub: sub ?? this.sub,
      );

  bool get usable => file.isNotEmpty && items > 0;
}

/// Kutayotgan / tasdiqlangan / rad etilgan amal (server) yoki hali
/// yuborilmagan amal (`queued`, telefonda).
class PackOp {
  final int id;
  final int pack;
  final String op;
  final String file;
  final int item;
  final String emoji;
  final int size;
  final String state;
  final String reason;

  const PackOp({
    required this.id,
    required this.pack,
    required this.op,
    this.file = '',
    this.item = 0,
    this.emoji = '',
    this.size = 0,
    required this.state,
    this.reason = '',
  });

  factory PackOp.fromJson(Map<String, dynamic> j) => PackOp(
        id: _i(j['id']),
        pack: _i(j['pack']),
        op: '${j['op'] ?? ''}',
        file: '${j['file'] ?? ''}',
        item: _i(j['item']),
        emoji: '${j['emoji'] ?? ''}',
        size: _i(j['size']),
        state: '${j['state'] ?? ''}',
        reason: '${j['reason'] ?? ''}',
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'pack': pack,
        'op': op,
        'file': file,
        'item': item,
        'emoji': emoji,
        'size': size,
        'state': state,
        'reason': reason,
      };

  bool get isAdd => op == 'add';
}

class PackItem {
  final int id;

  /// Ma'lumot boshiga nisbatan joylashuv.
  final int off, len, thumbOff, thumbLen;
  final bool animated;
  final int w, h;
  final String emoji;

  const PackItem({
    required this.id,
    required this.off,
    required this.len,
    required this.thumbOff,
    required this.thumbLen,
    required this.animated,
    required this.w,
    required this.h,
    required this.emoji,
  });

  factory PackItem.fromJson(Map<String, dynamic> j) => PackItem(
        id: _i(j['i']),
        off: _i(j['o']),
        len: _i(j['l']),
        thumbOff: _i(j['to']),
        thumbLen: _i(j['tl']),
        animated: _i(j['a']) == 1,
        w: _i(j['w']),
        h: _i(j['h']),
        emoji: '${j['e'] ?? ''}',
      );

  Map<String, dynamic> toJson() => {
        'i': id,
        'o': off,
        'l': len,
        'to': thumbOff,
        'tl': thumbLen,
        'a': animated ? 1 : 0,
        'w': w,
        'h': h,
        'e': emoji,
      };
}

class PackHeader {
  final int id;
  final String kind;
  final String title;
  final int ver;

  /// Faylda ma'lumot boshlanadigan joy (`16 + sarlavha uzunligi`).
  final int base;
  final List<PackItem> items;
  late final Map<int, PackItem> _byId = {for (final it in items) it.id: it};

  /// Kichik rasmlar bloki tugaydigan joy (ma'lumot boshiga nisbatan).
  late final int thumbEnd = items.fold<int>(0, (m, it) {
    final e = it.thumbOff + it.thumbLen;
    return e > m ? e : m;
  });

  PackHeader({
    required this.id,
    required this.kind,
    required this.title,
    required this.ver,
    required this.base,
    required this.items,
  });

  PackItem? find(int itemId) => _byId[itemId];

  factory PackHeader.fromJson(Map<String, dynamic> j, int base) => PackHeader(
        id: _i(j['id']),
        kind: '${j['kind'] ?? ''}',
        title: '${j['title'] ?? ''}',
        ver: _i(j['ver']),
        base: base,
        items: [
          for (final e in (j['items'] as List? ?? const []))
            if (e is Map) PackItem.fromJson(Map<String, dynamic>.from(e)),
        ],
      );

  Map<String, dynamic> toCache() => {
        'id': id,
        'kind': kind,
        'title': title,
        'ver': ver,
        'base': base,
        'items': [for (final it in items) it.toJson()],
      };

  static PackHeader? fromCache(Map<String, dynamic>? j) {
    if (j == null) return null;
    final base = _i(j['base']);
    if (base < 16) return null;
    return PackHeader.fromJson(j, base);
  }
}

/// Bitta element: to'plam + sarlavha + element.
class PackRef {
  final PackInfo info;
  final PackHeader header;
  final PackItem item;
  const PackRef(this.info, this.header, this.item);
}

/// Xabarga yuboriladigan tanlov.
class PackPick {
  final String kind;
  final int pack;
  final int item;
  final String emoji;
  const PackPick(this.kind, this.pack, this.item, this.emoji);

  /// Xabarning `media_file` maydoni.
  String get ref => 'pk_${pack}_$item';
}

/// `pk_<to'plam>_<element>` -> (to'plam, element).
(int, int)? parsePackRef(String name) {
  final m = RegExp(r'^pk_(\d{1,16})_(\d{1,9})$').firstMatch(name);
  if (m == null) return null;
  final p = int.parse(m.group(1)!), i = int.parse(m.group(2)!);
  return (p > 0 && i > 0) ? (p, i) : null;
}

/// Matn ichidagi maxsus emoji: `[pe:<to'plam>:<element>:<emoji>]`.
final RegExp kPackEmojiToken = RegExp(r'\[pe:(\d{1,16}):(\d{1,9}):([^\]]{0,16})\]');

String packEmojiToken(int pack, int item, String emoji) =>
    '[pe:$pack:$item:${emoji.replaceAll(RegExp(r'[\[\]]'), '')}]';

// ── YORDAMCHILAR ─────────────────────────────────────────────────

/// Bir vaqtda ko'pi bilan [max] ta ish.
class _Gate {
  final int max;
  int _n = 0;
  final List<Completer<void>> _q = [];
  _Gate(this.max);

  Future<T> run<T>(Future<T> Function() f) async {
    while (_n >= max) {
      final c = Completer<void>();
      _q.add(c);
      await c.future;
    }
    _n++;
    try {
      return await f();
    } finally {
      _n--;
      if (_q.isNotEmpty) _q.removeAt(0).complete();
    }
  }
}

/// Xotiradagi kesh (hajm bo'yicha cheklangan, eng eskisi chiqadi).
class _Lru {
  final int maxBytes;
  int _bytes = 0;
  final LinkedHashMap<String, Uint8List> _m = LinkedHashMap();
  _Lru(this.maxBytes);

  Uint8List? get(String k) {
    final v = _m.remove(k);
    if (v != null) _m[k] = v;
    return v;
  }

  void put(String k, Uint8List v) {
    final old = _m.remove(k);
    if (old != null) _bytes -= old.length;
    _m[k] = v;
    _bytes += v.length;
    while (_bytes > maxBytes && _m.length > 1) {
      final first = _m.keys.first;
      _bytes -= _m.remove(first)!.length;
    }
  }

  void clear() {
    _m.clear();
    _bytes = 0;
  }
}

/// Fayl turini BAYTLARIDAN aniqlaydi (kengaytmaga ishonilmaydi).
String sniffImage(List<int> b) {
  if (b.length >= 8 &&
      b[0] == 0x89 &&
      b[1] == 0x50 &&
      b[2] == 0x4E &&
      b[3] == 0x47) {
    return 'png';
  }
  if (b.length >= 3 && b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF) {
    return 'jpeg';
  }
  if (b.length >= 6 && b[0] == 0x47 && b[1] == 0x49 && b[2] == 0x46) {
    return 'gif';
  }
  if (b.length >= 12 &&
      b[0] == 0x52 &&
      b[1] == 0x49 &&
      b[2] == 0x46 &&
      b[3] == 0x46 &&
      b[8] == 0x57 &&
      b[9] == 0x45 &&
      b[10] == 0x42 &&
      b[11] == 0x50) {
    return 'webp';
  }
  return '';
}

// ═══════════════════════════════════════════════════════════════
//  XIZMAT
// ═══════════════════════════════════════════════════════════════

class PackService extends ChangeNotifier {
  PackService._();
  static final PackService instance = PackService._();

  static const String _libKey = 'pack_library';
  static const Duration _libMaxAge = Duration(minutes: 10);

  // ── KUTUBXONA (mening to'plamlarim, obunalar, amallar) ──────

  List<PackInfo> _mine = [];
  List<PackInfo> _subs = [];
  List<PackOp> _ops = [];
  bool _loaded = false;
  bool _loading = false;
  String? error;

  /// Obuna bo'lingan, lekin server hali bilmagan to'plamlar (darhol
  /// ko'rinsin).
  final Map<int, PackInfo> _localSubs = {};

  bool get loading => _loading;
  bool get loaded => _loaded;

  /// Diskdan darhol, kerak bo'lsa tarmoqdan yangilaydi.
  Future<void> load({bool force = false}) async {
    if (_loading) return;
    if (AuthService.instance.sessionToken == null) return;
    if (!_loaded) {
      final c = DiskCache.readOne(_libKey);
      if (c != null) {
        _apply(c);
        _loaded = true;
        notifyListeners();
      }
    }
    if (!force && _loaded && !DiskCache.isStale(_libKey, _libMaxAge)) return;
    _loading = true;
    error = null;
    notifyListeners();
    try {
      final r = await http
          .get(Uri.parse('$kApiBase/api/packs/library'), headers: _auth())
          .timeout(const Duration(seconds: 20));
      if (r.statusCode == 200) {
        final j = jsonDecode(r.body) as Map<String, dynamic>;
        _apply(j);
        _loaded = true;
        DiskCache.writeOne(_libKey, {
          'mine': j['mine'] ?? const [],
          'subs': j['subs'] ?? const [],
          'ops': j['ops'] ?? const [],
        });
        for (final p in [..._mine, ..._subs]) {
          _known[p.id] = _Known(p, DateTime.now());
        }
        _localSubs.removeWhere((id, _) => _subs.any((p) => p.id == id));
      } else {
        error = 'Yuklab bo\'lmadi (${r.statusCode})';
      }
    } catch (_) {
      error = 'Internet yo\'q';
    }
    _loading = false;
    notifyListeners();
  }

  void _apply(Map<String, dynamic> j) {
    List<T> list<T>(String k, T Function(Map<String, dynamic>) f) => [
          for (final e in (j[k] as List? ?? const []))
            if (e is Map) f(Map<String, dynamic>.from(e)),
        ];
    _mine = list('mine', PackInfo.fromJson);
    _subs = list('subs', PackInfo.fromJson);
    _ops = list('ops', PackOp.fromJson);
  }

  Map<String, String> _auth({bool json = false}) => {
        'Authorization': 'Bearer ${AuthService.instance.sessionToken ?? ''}',
        if (json) 'Content-Type': 'application/json',
      };

  /// Hisob almashganda / chiqilganda.
  void reset() {
    _mine = [];
    _subs = [];
    _ops = [];
    _loaded = false;
    _localSubs.clear();
    _known.clear();
    _headers.clear();
    _mem.clear();
    notifyListeners();
  }

  // ── NAVBATDAGI (hali yuborilmagan) AMALLAR USTIGA QO'YILADI ─

  Set<int> get _pendingDeleted => {
        for (final m in SyncQueue.instance.pendingPacks())
          if (m['op'] == 'delete') _i(m['pack']),
      };

  /// Mening to'plamlarim (yuborilmaganlari ham).
  List<PackInfo> get myPacks {
    final gone = _pendingDeleted;
    final out = _mine.where((p) => !gone.contains(p.id)).toList();
    for (final m in SyncQueue.instance.pendingPacks()) {
      if (m['op'] != 'new') continue;
      final id = _i(m['id']);
      if (id <= 0 || gone.contains(id) || out.any((p) => p.id == id)) continue;
      out.insert(
        0,
        PackInfo(
          id: id,
          kind: '${m['kind']}',
          title: '${m['title']}',
          ownerId: AuthService.instance.user?.id ?? 0,
        ),
      );
    }
    return out;
  }

  /// Obuna bo'lgan to'plamlarim.
  List<PackInfo> get subPacks {
    final off = <int>{};
    final on = <int>{};
    for (final m in SyncQueue.instance.pendingPacks()) {
      if (m['op'] != 'sub') continue;
      (m['on'] == false ? off : on).add(_i(m['pack']));
    }
    final out = _subs.where((p) => !off.contains(p.id)).toList();
    for (final e in _localSubs.values) {
      if (on.contains(e.id) && !out.any((p) => p.id == e.id)) out.add(e);
    }
    return out;
  }

  bool isSubscribed(int packId) => subPacks.any((p) => p.id == packId);

  /// Ishlatish mumkin bo'lgan (elementi bor) to'plamlar — turi bo'yicha.
  List<PackInfo> usable(String kind) => [
        for (final p in [...myPacks, ...subPacks])
          if (p.kind == kind && p.usable) p,
      ];

  /// To'plamning amallari: serverdagi + hali yuborilmagan (`queued`).
  List<PackOp> opsOf(int packId) {
    final out = _ops.where((o) => o.pack == packId).toList();
    for (final m in SyncQueue.instance.pendingPacks()) {
      if (_i(m['pack']) != packId) continue;
      if (m['op'] == 'add') {
        out.add(PackOp(
          id: -1,
          pack: packId,
          op: 'add',
          file: '${m['file']}',
          emoji: '${m['emoji'] ?? ''}',
          size: _i(m['size']),
          state: 'queued',
        ));
      }
    }
    return out;
  }

  /// Olib tashlash so'ralgan (hali bajarilmagan) elementlar — ekranda yashirinadi.
  Set<int> removedItems(int packId) => {
        for (final m in SyncQueue.instance.pendingPacks())
          if (m['op'] == 'remove' && _i(m['pack']) == packId) _i(m['item']),
        for (final o in _ops)
          if (o.pack == packId && o.op == 'remove' && o.state != 'rejected')
            o.item,
      };

  // ── AMALLAR (hammasi SyncQueue orqali) ───────────────────────

  static final Random _rnd = Random.secure();

  int _newPackId() => (_rnd.nextInt(0x7fffffff) + 1) * 2097152 + _rnd.nextInt(2097152);

  void _flushSoon() {
    unawaited(() async {
      try {
        await SyncQueue.instance.flush(force: true);
      } catch (_) {}
      await load(force: true);
    }());
  }

  /// Yangi to'plam. Nom bo'sh yoki juda uzun bo'lsa `null`.
  PackInfo? createPack(String kind, String title) {
    final t = title.trim();
    if (!PackKind.all.contains(kind) ||
        t.isEmpty ||
        t.length > kPackTitleMax ||
        myPacks.length >= kPackMaxPerUser) {
      return null;
    }
    final id = _newPackId();
    SyncQueue.instance.putPack('p:new:$id', {
      'op': 'new',
      'id': id,
      'kind': kind,
      'title': t,
    });
    notifyListeners();
    _flushSoon();
    return PackInfo(
      id: id,
      kind: kind,
      title: t,
      ownerId: AuthService.instance.user?.id ?? 0,
    );
  }

  void deletePack(int id) {
    SyncQueue.instance.putPack('p:del:$id', {'op': 'delete', 'pack': id});
    notifyListeners();
    _flushSoon();
  }

  void removeItem(int packId, int itemId) {
    SyncQueue.instance.putPack('p:rm:$packId:$itemId',
        {'op': 'remove', 'pack': packId, 'item': itemId});
    notifyListeners();
    _flushSoon();
  }

  void setSubscribed(PackInfo p, bool on) {
    if (on) _localSubs[p.id] = p.copyWith(sub: true);
    SyncQueue.instance
        .putPack('p:sub:${p.id}', {'op': 'sub', 'pack': p.id, 'on': on});
    notifyListeners();
    _flushSoon();
  }

  /// Elementni yuklaydi (admin ko'rib chiqishi uchun). Muvaffaqiyatda `null`,
  /// aks holda xato matni.
  Future<String?> addItem(
    PackInfo pack,
    String path, {
    String emoji = '',
    void Function(int sent, int total)? onProgress,
  }) async {
    final uid = AuthService.instance.user?.id ?? 0;
    if (uid <= 0) return 'Avval hisobga kiring';
    final waiting = [
      for (final p in myPacks)
        for (final o in opsOf(p.id))
          if (o.isAdd && (o.state == 'pending' || o.state == 'queued')) o,
    ].length;
    if (waiting >= kPackMaxWaiting) {
      return 'Ko\'rib chiqilayotgan rasmlar juda ko\'p ($kPackMaxWaiting ta). '
          'Admin ko\'rib chiqquncha kuting';
    }
    final f = File(path);
    if (!await f.exists()) return 'Fayl topilmadi';
    final size = await f.length();
    if (size <= 0) return 'Fayl bo\'sh';
    if (size > kPackItemMaxBytes) {
      return 'Fayl 5 MB dan katta (${(size / 1048576).toStringAsFixed(1)} MB)';
    }
    final head = await f.openRead(0, 16).fold<List<int>>([], (a, b) => a..addAll(b));
    if (sniffImage(head).isEmpty) {
      return 'Faqat PNG, JPG, GIF yoki WebP rasm yuklash mumkin';
    }
    if (!TelegramService.instance.authorized) {
      return 'Fayl Telegram orqali yuklanadi — avval Telegram hisobini ulang';
    }
    final name = 'pki_${uid}_${DateTime.now().millisecondsSinceEpoch}'
        '_${_rnd.nextInt(0xffff).toRadixString(16)}.bin';
    final err = await TelegramService.instance.uploadFile(
      path,
      name,
      'application/octet-stream',
      onProgress: onProgress,
    );
    if (err != null) return err;
    final e = emoji.replaceAll(RegExp(r'[\[\]]'), '').characters.take(3).toString();
    SyncQueue.instance.putPack('p:add:$name', {
      'op': 'add',
      'pack': pack.id,
      'file': name,
      'emoji': e,
      'size': size,
    });
    notifyListeners();
    _flushSoon();
    return null;
  }

  // ── OMMAVIY TO'PLAMLAR ───────────────────────────────────────

  /// Ommaviy to'plamlar (yangilaridan). [before] — sahifalash.
  Future<List<PackInfo>?> browse({String kind = '', int before = 0}) async {
    try {
      final q = [
        if (kind.isNotEmpty) 'kind=$kind',
        if (before > 0) 'before=$before',
      ].join('&');
      final r = await http
          .get(Uri.parse('$kApiBase/api/packs/public${q.isEmpty ? '' : '?$q'}'),
              headers: _auth())
          .timeout(const Duration(seconds: 20));
      if (r.statusCode != 200) return null;
      final j = jsonDecode(r.body) as Map<String, dynamic>;
      final out = [
        for (final e in (j['packs'] as List? ?? const []))
          if (e is Map) PackInfo.fromJson(Map<String, dynamic>.from(e)),
      ];
      // Sahifalash uchun oxirgi qatorning vaqti kerak.
      _browseTimes.addAll({
        for (final e in (j['packs'] as List? ?? const []))
          if (e is Map) _i(e['id']): _i(e['created_at']),
      });
      for (final p in out) {
        _known[p.id] = _Known(p, DateTime.now());
      }
      return out;
    } catch (_) {
      return null;
    }
  }

  final Map<int, int> _browseTimes = {};

  /// Ommaviy ro'yxatda shu to'plam qachon yaratilgan (sahifalash kursori).
  int createdAtOf(int packId) => _browseTimes[packId] ?? 0;

  // ── ADMIN ────────────────────────────────────────────────────

  Future<List<Map<String, dynamic>>?> adminPending() async {
    try {
      final r = await http
          .get(Uri.parse('$kApiBase/api/packs/admin/pending'), headers: _auth())
          .timeout(const Duration(seconds: 20));
      if (r.statusCode != 200) return null;
      final j = jsonDecode(r.body) as Map<String, dynamic>;
      return [
        for (final e in (j['ops'] as List? ?? const []))
          if (e is Map) Map<String, dynamic>.from(e),
      ];
    } catch (_) {
      return null;
    }
  }

  Future<String?> adminReview(List<int> ids, bool approve,
      {String reason = ''}) async {
    try {
      final r = await http
          .post(Uri.parse('$kApiBase/api/packs/admin/review'),
              headers: _auth(json: true),
              body: jsonEncode(
                  {'ids': ids, 'approve': approve, 'reason': reason}))
          .timeout(const Duration(seconds: 25));
      if (r.statusCode == 200) return null;
      return '${(jsonDecode(r.body) as Map)['error'] ?? 'Xato (${r.statusCode})'}';
    } catch (_) {
      return 'Internet yo\'q';
    }
  }

  Future<String?> adminDeletePack(int packId) async {
    try {
      final r = await http
          .post(Uri.parse('$kApiBase/api/packs/admin/delete'),
              headers: _auth(json: true), body: jsonEncode({'pack': packId}))
          .timeout(const Duration(seconds: 25));
      if (r.statusCode == 200) return null;
      return 'Xato (${r.statusCode})';
    } catch (_) {
      return 'Internet yo\'q';
    }
  }

  /// Ko'rib chiqilayotgan (hali to'plamga qo'shilmagan) element rasmi.
  Future<Uint8List?> stagingBytes(String file) =>
      TelegramService.instance.fetchBytes(file);

  // ═════════════════════════════════════════════════════════════
  //  TO'PLAM MA'LUMOTI (fayl nomi, versiya) VA SARLAVHA
  // ═════════════════════════════════════════════════════════════

  final Map<int, _Known> _known = {};
  final Map<int, PackHeader> _headers = {};
  final Map<int, Future<PackHeader?>> _headerFlight = {};
  final Map<int, List<Completer<PackInfo?>>> _infoWaiters = {};
  Timer? _infoTimer;

  /// To'plam ma'lumoti. Xabarda uchragan begona to'plam ham (server
  /// `/api/packs/info` dan, bir necha so'rov bitta bo'lib).
  Future<PackInfo?> infoFor(int packId, {bool refresh = false}) {
    final k = _known[packId];
    if (!refresh && k != null) return Future.value(k.info);
    for (final p in [..._mine, ..._subs]) {
      if (p.id == packId && !refresh) return Future.value(p);
    }
    final c = Completer<PackInfo?>();
    _infoWaiters.putIfAbsent(packId, () => []).add(c);
    _infoTimer ??= Timer(const Duration(milliseconds: 40), _flushInfo);
    return c.future;
  }

  Future<void> _flushInfo() async {
    _infoTimer = null;
    final waiters = Map.of(_infoWaiters);
    _infoWaiters.clear();
    if (waiters.isEmpty) return;
    final found = <int, PackInfo>{};
    try {
      final ids = waiters.keys.take(20).toList();
      final r = await http
          .get(Uri.parse('$kApiBase/api/packs/info?ids=${ids.join(',')}'),
              headers: _auth())
          .timeout(const Duration(seconds: 20));
      if (r.statusCode == 200) {
        final j = jsonDecode(r.body) as Map<String, dynamic>;
        for (final e in (j['packs'] as List? ?? const [])) {
          if (e is Map) {
            final p = PackInfo.fromJson(Map<String, dynamic>.from(e));
            found[p.id] = p;
            _known[p.id] = _Known(p, DateTime.now());
          }
        }
      }
    } catch (_) {}
    for (final e in waiters.entries) {
      for (final c in e.value) {
        if (!c.isCompleted) c.complete(found[e.key]);
      }
    }
    // 20 tadan ortig'i keyingi turda.
    if (waiters.length > 20) {
      for (final id in waiters.keys.skip(20)) {
        for (final c in waiters[id]!) {
          unawaited(infoFor(id).then((v) {
            if (!c.isCompleted) c.complete(v);
          }));
        }
      }
    }
  }

  /// Sarlavha (elementlar ro'yxati). Diskdan yoki fayl boshidan.
  Future<PackHeader?> header(PackInfo info) {
    if (info.file.isEmpty) return Future.value(null);
    final mem = _headers[info.id];
    if (mem != null && mem.ver >= info.version) return Future.value(mem);
    return _headerFlight[info.id] ??=
        _loadHeader(info).whenComplete(() => _headerFlight.remove(info.id));
  }

  Future<PackHeader?> _loadHeader(PackInfo info) async {
    final disk = PackHeader.fromCache(DiskCache.readOne('pack_hdr_${info.id}'));
    if (disk != null && disk.ver >= info.version) {
      _headers[info.id] = disk;
      return disk;
    }
    // Fayl boshidan 64 KB: preamble (16 bayt) + sarlavha odatda sig'adi.
    final first = await _range(info.file, 0, 65536);
    if (first == null || first.length < 16) return disk;
    final bd = ByteData.sublistView(first);
    if (first[0] != 0x41 || first[1] != 0x52 || first[2] != 0x55 || first[3] != 0x50) {
      return disk; // "ARUP" emas
    }
    final hlen = bd.getUint32(8, Endian.big);
    if (hlen <= 0 || hlen > 16 * 1024 * 1024) return disk;
    var bytes = first;
    if (16 + hlen > first.length) {
      final rest = await _range(info.file, first.length, 16 + hlen - first.length);
      if (rest == null) return disk;
      bytes = Uint8List.fromList([...first, ...rest]);
    }
    try {
      final j = jsonDecode(utf8.decode(bytes.sublist(16, 16 + hlen)))
          as Map<String, dynamic>;
      final h = PackHeader.fromJson(j, 16 + hlen);
      if (h.id != info.id) return disk;
      _headers[info.id] = h;
      DiskCache.writeOne('pack_hdr_${info.id}', h.toCache());
      return h;
    } catch (_) {
      return disk;
    }
  }

  /// To'plam + sarlavha + element. Element sarlavhada yo'q bo'lsa
  /// (to'plam yangilangan) — ma'lumot bir marta yangilanadi.
  Future<PackRef?> resolve(int packId, int itemId) async {
    var info = await infoFor(packId);
    if (info == null) return null;
    var h = await header(info);
    var it = h?.find(itemId);
    if (it == null) {
      final fresh = await infoFor(packId, refresh: true);
      if (fresh != null && fresh.version > info.version) {
        info = fresh;
        h = await header(info);
        it = h?.find(itemId);
      }
    }
    if (h == null || it == null) return null;
    return PackRef(info, h, it);
  }

  // ═════════════════════════════════════════════════════════════
  //  BAYTLAR: XOTIRA -> DISK (SHIFRLANGAN) -> TELEGRAM (BO'LAK)
  // ═════════════════════════════════════════════════════════════

  final _Lru _mem = _Lru(24 * 1024 * 1024);
  final _Gate _gate = _Gate(4);
  final Map<String, Future<Uint8List?>> _flight = {};
  final Map<String, DateTime> _failed = {};
  static const Duration _cool = Duration(seconds: 20);

  bool _coolingDown(String k) {
    final t = _failed[k];
    if (t == null) return false;
    if (DateTime.now().difference(t) > _cool) {
      _failed.remove(k);
      return false;
    }
    return true;
  }

  /// Statik kichik rasm (~5 KB).
  Future<Uint8List?> thumb(PackRef r) => _thumb(r.info, r.header, r.item);

  /// Elementning o'zi (animatsiya bo'lishi mumkin).
  Future<Uint8List?> data(PackRef r) => _data(r.info, r.header, r.item);

  Future<Uint8List?> _thumb(PackInfo info, PackHeader h, PackItem it) async {
    final key = 't${info.id}_${it.id}';
    final m = _mem.get(key);
    if (m != null) return m;
    if (_coolingDown(key)) return null;
    final d = await _diskGet(info.id, key);
    if (d != null) {
      _mem.put(key, d);
      return d;
    }
    // Bitta so'rovda yonma-yon turgan bir necha kichik rasm olinadi.
    const w = 256 * 1024;
    final ws = (it.thumbOff ~/ w) * w;
    var we = ws + w;
    if (it.thumbOff + it.thumbLen > we) we = it.thumbOff + it.thumbLen;
    if (we > h.thumbEnd) we = h.thumbEnd;
    final wk = 'w${info.id}_${info.version}_$ws';
    final blob = await (_flight[wk] ??= _range(info.file, h.base + ws, we - ws)
        .whenComplete(() => _flight.remove(wk)));
    if (blob == null || blob.length < we - ws) {
      _failed[key] = DateTime.now();
      return null;
    }
    Uint8List? mine;
    for (final o in h.items) {
      if (o.thumbLen <= 0 || o.thumbOff < ws || o.thumbOff + o.thumbLen > we) continue;
      final piece = Uint8List.sublistView(
          blob, o.thumbOff - ws, o.thumbOff - ws + o.thumbLen);
      final k = 't${info.id}_${o.id}';
      _mem.put(k, piece);
      unawaited(_diskPut(info.id, k, piece));
      if (o.id == it.id) mine = piece;
    }
    return mine;
  }

  Future<Uint8List?> _data(PackInfo info, PackHeader h, PackItem it) async {
    final key = 'd${info.id}_${it.id}';
    final m = _mem.get(key);
    if (m != null) return m;
    if (_coolingDown(key)) return null;
    return _flight[key] ??= () async {
      try {
        final d = await _diskGet(info.id, key);
        if (d != null) {
          _mem.put(key, d);
          return d;
        }
        final b = await _range(info.file, h.base + it.off, it.len);
        if (b == null || b.length != it.len) {
          _failed[key] = DateTime.now();
          return null;
        }
        _mem.put(key, b);
        unawaited(_diskPut(info.id, key, b));
        return b;
      } finally {
        _flight.remove(key);
      }
    }();
  }

  // ── TELEGRAM'DAN BO'LAK ──────────────────────────────────────

  final Map<String, Object> _holdOwners = {};
  final Map<String, Timer> _holdTimers = {};

  /// To'plam fayli bot chatida turaversin (har o'qishda qayta so'ralmasin):
  /// oxirgi o'qishdan 45 soniya keyin qo'yib yuboriladi.
  void _touch(String file) {
    final tg = TelegramService.instance;
    final owner = _holdOwners.putIfAbsent(file, () => Object());
    tg.hold(owner, file);
    _holdTimers[file]?.cancel();
    _holdTimers[file] = Timer(const Duration(seconds: 45), () {
      tg.unhold(owner);
      _holdOwners.remove(file);
      _holdTimers.remove(file);
    });
  }

  /// Fayldan [len] bayt (`offset` dan) — mahalliy Telegram manbasidan,
  /// allaqachon ochilgan holda. Bo'lmasa `null`.
  Future<Uint8List?> _range(String file, int offset, int len) {
    if (len <= 0) return Future.value(Uint8List(0));
    return _gate.run(() async {
      final tg = TelegramService.instance;
      try {
        _touch(file);
        final url = await tg.prepare(file);
        if (url == null) return null;
        final r = await http
            .get(Uri.parse(url),
                headers: {'Range': 'bytes=$offset-${offset + len - 1}'})
            .timeout(const Duration(seconds: 45));
        if (r.statusCode == 206 ||
            (r.statusCode == 200 && r.bodyBytes.length == len)) {
          return r.bodyBytes;
        }
        tg.invalidate(file);
        return null;
      } catch (_) {
        tg.invalidate(file);
        return null;
      }
    });
  }

  // ── DISK (shifrlangan) ───────────────────────────────────────

  static const String _label = 'pack_item';

  String? _root() {
    final r = RustCore.instance.rootDirPath;
    return r == null ? null : '$r/aru_packs';
  }

  File? _file(int packId, String key) {
    final r = _root();
    return r == null ? null : File('$r/$packId/$key');
  }

  Future<Uint8List?> _diskGet(int packId, String key) async {
    try {
      final f = _file(packId, key);
      if (f == null || !await f.exists()) return null;
      return RustCore.instance.openBytes(_label, await f.readAsBytes());
    } catch (_) {
      return null;
    }
  }

  Future<void> _diskPut(int packId, String key, Uint8List b) async {
    try {
      final f = _file(packId, key);
      if (f == null) return;
      final sealed = RustCore.instance.sealBytes(_label, b);
      if (sealed == null) return; // shifrlash yo'q — ochiq yozilmaydi
      await f.parent.create(recursive: true);
      await f.writeAsBytes(sealed);
    } catch (_) {}
  }

  /// Diskdagi va xotiradagi hamma to'plam keshi (sozlamalardagi tozalash).
  Future<void> clearCache() async {
    _mem.clear();
    _headers.clear();
    try {
      final r = _root();
      if (r != null && await Directory(r).exists()) {
        await Directory(r).delete(recursive: true);
      }
    } catch (_) {}
  }

  /// Diskdagi kesh hajmi (bayt).
  Future<int> cacheBytes() async {
    var total = 0;
    try {
      final r = _root();
      if (r == null) return 0;
      final dir = Directory(r);
      if (!await dir.exists()) return 0;
      await for (final e in dir.list(recursive: true, followLinks: false)) {
        if (e is File) total += await e.length();
      }
    } catch (_) {}
    return total;
  }

  // ── YAQINDA ISHLATILGANLAR ───────────────────────────────────

  final Map<String, List<PackPick>> _recent = {};

  List<PackPick> recent(String kind) {
    final have = _recent[kind];
    if (have != null) return List.of(have);
    final rows = DiskCache.read('pack_recent_$kind') ?? const [];
    final out = [
      for (final m in rows)
        PackPick(kind, _i(m['p']), _i(m['i']), '${m['e'] ?? ''}'),
    ];
    _recent[kind] = out;
    return List.of(out);
  }

  void noteRecent(PackPick p) {
    final l = _recent[p.kind] ??= recent(p.kind);
    l.removeWhere((e) => e.pack == p.pack && e.item == p.item);
    l.insert(0, p);
    if (l.length > 30) l.removeLast();
    DiskCache.write('pack_recent_${p.kind}', [
      for (final e in l) {'p': e.pack, 'i': e.item, 'e': e.emoji},
    ]);
  }
}

class _Known {
  final PackInfo info;
  final DateTime at;
  _Known(this.info, this.at);
}
