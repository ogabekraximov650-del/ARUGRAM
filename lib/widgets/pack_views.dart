// lib/widgets/pack_views.dart — TO'PLAM ELEMENTLARINI KO'RSATISH.
//
// Telefonga bosim tushmasligi uchun (`pack_service.dart` boshidagi izoh):
//
//   * ro'yxat va to'plam oynasida faqat kichik STATIK rasm (`PackImage`,
//     `animate: false`);
//   * xabarda va matn ichida animatsiya faqat bir vaqtda cheklangan sondagi
//     joyda (`AnimSlots`) — qolgani statik turadi. Telefon kuchsizroq
//     bo'lsa chegara kichikroq (`DevicePerf`);
//   * ekrandan chiqqan vidjet o'z o'rnini qaytaradi;
//   * rasm o'z o'lchamida dekodlanadi (`cacheWidth`) — 512 px lik rasm
//     24 dp lik joyda katta xotira olmaydi.

import 'dart:async';
import 'dart:io';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show RenderAbstractViewport;
import 'package:video_player/video_player.dart';

import '../services/device_perf.dart';
import '../screens/pack_detail_screen.dart';
import '../services/pack_service.dart';
import 'media_placeholder.dart';

/// Bir vaqtda nechta animatsiya ishlashi mumkin (telefonga bosim tushmasin).
///
/// Uch hovuz: KICHIK (emoji va boshqa <= 200 KB — arzon, ko'pi mumkin),
/// KATTA (stiker/GIF) va VIDEO (ovozli MP4 — qurilma dekoderi kam, 1-2 ta).
/// Joy bo'lmasa kichik statik rasm turadi, joy bo'shaganda animatsiya boshlanadi.
enum AnimPool { small, big, video }

class AnimSlots extends ChangeNotifier {
  AnimSlots._();
  static final AnimSlots instance = AnimSlots._();

  final Map<Object, AnimPool> _used = {};

  int _cap(AnimPool p) => switch (p) {
        AnimPool.small => switch (DevicePerf.cls) {
            PerfClass.low => 12,
            PerfClass.average => 30,
            PerfClass.high => 60,
          },
        AnimPool.big => switch (DevicePerf.cls) {
            PerfClass.low => 4,
            PerfClass.average => 8,
            PerfClass.high => 14,
          },
        AnimPool.video => DevicePerf.cls == PerfClass.low ? 1 : 2,
      };

  bool tryAcquire(Object owner, [AnimPool pool = AnimPool.big]) {
    if (_used.containsKey(owner)) return true;
    final n = _used.values.where((v) => v == pool).length;
    if (n >= _cap(pool)) return false;
    _used[owner] = pool;
    return true;
  }

  /// Joy bo'shadi — kutayotganlar qayta urinadi (animatsiya keyin boshlanadi).
  void release(Object owner) {
    if (_used.remove(owner) != null) notifyListeners();
  }
}

/// Ovozli GIF: bir vaqtda faqat bittasida ovoz yoqiq. Ovoz yoqilsa asosiy
/// pleyer pauza bo'ladi, pleyerda play bosilsa ovoz o'chadi (animatsiya
/// to'xtamaydi).
class PackSoundHub extends ChangeNotifier {
  PackSoundHub._();
  static final PackSoundHub instance = PackSoundHub._();

  Object? _owner;
  bool get active => _owner != null;
  bool isOn(Object o) => identical(_owner, o);

  void on(Object owner) {
    if (identical(_owner, owner)) return;
    _owner = owner;
    notifyListeners();
  }

  void off(Object owner) {
    if (!identical(_owner, owner)) return;
    _owner = null;
    notifyListeners();
  }

  /// Pleyerda play bosildi — GIF ovozi o'chadi.
  void muteAll() {
    if (_owner == null) return;
    _owner = null;
    notifyListeners();
  }
}

/// Bitta element rasmi.
///
/// [animate] `false` — faqat kichik statik rasm. `true` — elementning o'zi;
/// animatsiyali bo'lsa va bo'sh joy ([AnimSlots]) bo'lsa animatsiya
/// ishlaydi, aks holda statik rasm turadi.
class PackImage extends StatefulWidget {
  final int pack;
  final int item;
  final double size;

  /// Balandlik (berilmasa [size] — kvadrat). GIF devorida turli nisbat.
  final double? height;
  final bool animate;

  /// Ovozli video elementlarda ovoz yoqilsinmi (chatda — o'chiq, ko'rishda — yoqiq).
  final bool sound;
  final BoxFit fit;

  /// Yuklanmasa (yoki to'plam topilmasa) ko'rsatiladigan narsa.
  final Widget? fallback;

  const PackImage({
    super.key,
    required this.pack,
    required this.item,
    required this.size,
    this.height,
    this.animate = false,
    this.sound = false,
    this.fit = BoxFit.contain,
    this.fallback,
  });

  @override
  State<PackImage> createState() => _PackImageState();
}

class _PackImageState extends State<PackImage> {
  Uint8List? _bytes;
  bool _failed = false;
  bool _slot = false;
  bool _waitingSlot = false;
  int _gen = 0;
  int _retries = 0;
  Timer? _retry;
  PackRef? _ref;
  VideoPlayerController? _vc;
  File? _vfile;
  AnimPool _pool = AnimPool.big;
  Uint8List? _thumbBytes;
  ScrollPosition? _scrollPos;
  Timer? _visTimer;

  /// Elementning o'zi shu hajmdan katta bo'lsa, kichik rasm o'rniga
  /// yuklanmaydi (devorda bir vaqtda ko'p og'ir fayl olinmasin).
  static const int _fallbackMax = 300 * 1024;

  @override
  void initState() {
    super.initState();
    _startSoon();
  }

  bool _pendingLoad = false;
  Timer? _nearTimer;

  /// Joylashgach: element ekran va undan 2 qator pastda/tepada bo'lsa
  /// yuklanadi, aks holda aylantirilganda (tez) yuklanadi.
  void _startSoon() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      if (_withinWindow()) {
        _pendingLoad = false;
        unawaited(_load());
      } else {
        _pendingLoad = true;
      }
    });
  }

  /// Element aylanuvchi ro'yxatning ko'rinadigan qismidan har tomonga 2 qator
  /// (elementning o'z balandligi x 2) ichidami. Ro'yxatdan tashqarida — doim.
  bool _withinWindow() {
    final ro = context.findRenderObject();
    final pos = _scrollPos;
    if (ro is! RenderBox || !ro.attached || !ro.hasSize) return true;
    final vp = RenderAbstractViewport.maybeOf(ro);
    if (vp == null || pos == null || !pos.hasPixels) return true;
    try {
      final start = vp.getOffsetToReveal(ro, 0.0).offset;
      final extent = pos.axis == Axis.vertical ? ro.size.height : ro.size.width;
      final end = start + extent;
      final margin = 2 * extent;
      return end >= pos.pixels - margin &&
          start <= pos.pixels + pos.viewportDimension + margin;
    } catch (_) {
      return true;
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final pos = Scrollable.maybeOf(context)?.position;
    if (!identical(pos, _scrollPos)) {
      _scrollPos?.removeListener(_onScroll);
      _scrollPos = pos;
      pos?.addListener(_onScroll);
    }
  }

  /// Element ekranda TO'LIQ ko'rinyaptimi (aylanuvchi ro'yxatda). Ro'yxatdan
  /// tashqarida (xabar, matn ichida) — doim ko'rinadi deb olinadi.
  bool _fullyVisible() {
    final ro = context.findRenderObject();
    if (ro is! RenderBox || !ro.attached) return false;
    final vp = RenderAbstractViewport.maybeOf(ro);
    final pos = _scrollPos;
    if (vp == null || pos == null || !pos.hasPixels) return true;
    try {
      final lead = vp.getOffsetToReveal(ro, 0.0).offset;
      final trail = vp.getOffsetToReveal(ro, 1.0).offset;
      const eps = 1.0;
      return pos.pixels <= lead + eps && pos.pixels >= trail - eps;
    } catch (_) {
      return true; // hali joylashmagan — keyingi tekshiruvda aniqlanadi
    }
  }

  void _onScroll() {
    // Yuklanmagan element oynaga kirsa — aylantirish DAVOMIDA (tez) yuklanadi.
    if (_pendingLoad && _nearTimer == null) {
      _nearTimer = Timer(const Duration(milliseconds: 60), () {
        _nearTimer = null;
        if (mounted && _pendingLoad && _withinWindow()) {
          _pendingLoad = false;
          unawaited(_load());
        }
      });
    }
    _visTimer?.cancel();
    _visTimer = Timer(const Duration(milliseconds: 140), _recheck);
  }

  /// Aylantirish to'xtagach: to'liq ko'ringanlar animatsiyani boshlaydi,
  /// ko'rinmay qolganlar joyini bo'shatib statik rasmga qaytadi.
  void _recheck() {
    if (!mounted || !widget.animate) return;
    final r = _ref;
    if (r == null || !r.item.animated) return;
    if (_fullyVisible()) {
      if (!_slot && !_waitingSlot) unawaited(_loadFull(r, _gen));
    } else if (_slot || _waitingSlot) {
      _stopWaiting();
      _release();
      _dropVideo();
      final t = _thumbBytes;
      if (t != null) setState(() => _bytes = t);
    }
  }

  @override
  void didUpdateWidget(PackImage old) {
    super.didUpdateWidget(old);
    if (old.sound != widget.sound)
      unawaited(_vc?.setVolume(widget.sound ? 1 : 0));
    if (old.pack != widget.pack ||
        old.item != widget.item ||
        old.animate != widget.animate) {
      _reset();
      _startSoon();
    }
  }

  @override
  void dispose() {
    _gen++;
    _retry?.cancel();
    _visTimer?.cancel();
    _nearTimer?.cancel();
    _scrollPos?.removeListener(_onScroll);
    _reset(keepBytes: true);
    super.dispose();
  }

  void _dropVideo() {
    final c = _vc;
    final f = _vfile;
    _vc = null;
    _vfile = null;
    if (c != null) unawaited(c.dispose());
    if (f != null) {
      try {
        f.deleteSync();
      } catch (_) {}
    }
  }

  void _reset({bool keepBytes = false}) {
    _dropVideo();
    _retry?.cancel();
    _stopWaiting();
    _release();
    _ref = null;
    _retries = 0;
    if (!keepBytes) {
      _bytes = null;
      _failed = false;
    }
  }

  void _release() {
    if (_slot) {
      AnimSlots.instance.release(this);
      _slot = false;
    }
  }

  void _stopWaiting() {
    if (_waitingSlot) {
      AnimSlots.instance.removeListener(_onSlotFree);
      _waitingSlot = false;
    }
  }

  /// Internet yo'q yoki to'plam hali ochilmadi — bir necha marta qayta uriniladi
  /// (xabar abadiy bo'sh qolib ketmasin).
  void _scheduleRetry() {
    if (!mounted || _retries >= 3) return;
    _retries++;
    _retry?.cancel();
    _retry = Timer(Duration(seconds: 6 * _retries), () {
      if (!mounted) return;
      if (_bytes == null) {
        unawaited(_load());
      } else if (widget.animate && _ref != null && !_slot && !_waitingSlot) {
        unawaited(_loadFull(_ref!, _gen));
      }
    });
  }

  Future<void> _load() async {
    final gen = ++_gen;
    final svc = PackService.instance;
    final r = await svc.resolve(widget.pack, widget.item);
    if (!mounted || gen != _gen) return;
    if (r == null) {
      setState(() => _failed = true);
      _scheduleRetry();
      return;
    }
    _ref = r;
    // 1) tez: kichik statik rasm.
    final t = await svc.thumb(r);
    if (!mounted || gen != _gen) return;
    if (t != null) {
      _thumbBytes = t;
      setState(() {
        _bytes = t;
        _failed = false;
      });
    }
    if (!widget.animate) {
      if (t == null) {
        // Kichik rasm olinmadi — elementning o'zini sinab ko'ramiz (kichik
        // bo'lsa), rasm ko'rinmay qolmasin.
        if (!r.item.animated && r.item.len <= _fallbackMax) {
          final d = await svc.data(r);
          if (!mounted || gen != _gen) return;
          if (d != null) {
            setState(() {
              _bytes = d;
              _failed = false;
            });
            return;
          }
        }
        setState(() => _failed = true);
        _scheduleRetry();
      }
      return;
    }
    await _loadFull(r, gen);
  }

  /// Elementning o'zi. Animatsiya bo'lsa faqat bo'sh joy ([AnimSlots]) bo'lganda;
  /// bo'lmasa kichik statik rasm turadi va joy bo'shashini kutadi.
  Future<void> _loadFull(PackRef r, int gen) async {
    final svc = PackService.instance;
    // Animatsiya faqat to'liq ko'ringan elementda; aks holda aylantirish
    // to'xtaganda `_recheck` boshlaydi.
    if (r.item.animated && !_slot && !_fullyVisible()) return;
    if (r.item.animated && !_slot) {
      final pool = r.item.video
          ? AnimPool.video
          : (r.item.len <= 200 * 1024 ? AnimPool.small : AnimPool.big);
      _pool = pool;
      if (!AnimSlots.instance.tryAcquire(this, pool)) {
        if (!_waitingSlot) {
          _waitingSlot = true;
          AnimSlots.instance.addListener(_onSlotFree);
        }
        return;
      }
      _slot = true;
    }
    final d = await svc.data(r);
    if (!mounted || gen != _gen) return;
    if (d != null && r.item.video) {
      await _startVideo(r, d, gen);
      return;
    }
    if (d != null) {
      setState(() {
        _bytes = d;
        _failed = false;
      });
    } else {
      _release();
      if (_bytes == null) {
        setState(() => _failed = true);
      }
      // Kichik rasm turgan bo'lsa ham to'liq elementni qayta urinib ko'radi.
      _scheduleRetry();
    }
  }

  /// Ovozli MP4: vaqtinchalik faylga yoziladi va takrorlanib o'ynaydi.
  Future<void> _startVideo(PackRef r, Uint8List d, int gen) async {
    try {
      final f = File(
          '${Directory.systemTemp.path}/aru_pv_${r.info.id}_${r.item.id}.mp4');
      await f.writeAsBytes(d, flush: true);
      final c = VideoPlayerController.file(f);
      await c.initialize();
      if (!mounted || gen != _gen) {
        await c.dispose();
        try {
          f.deleteSync();
        } catch (_) {}
        return;
      }
      await c.setLooping(true);
      await c.setVolume(widget.sound ? 1 : 0);
      await c.play();
      setState(() {
        _vc = c;
        _vfile = f;
      });
    } catch (_) {
      _release();
    }
  }

  void _onSlotFree() {
    final r = _ref;
    if (!mounted || r == null || !_waitingSlot) return;
    if (AnimSlots.instance.tryAcquire(this, _pool)) {
      _stopWaiting();
      _slot = true;
      unawaited(_loadFull(r, _gen));
    }
  }

  @override
  Widget build(BuildContext context) {
    final b = _bytes;
    final vc = _vc;
    if (vc != null && vc.value.isInitialized) {
      return SizedBox(
        width: widget.size,
        height: widget.height ?? widget.size,
        child: FittedBox(
          fit: widget.fit == BoxFit.cover ? BoxFit.cover : BoxFit.contain,
          clipBehavior: Clip.hardEdge,
          child: SizedBox(
            width: vc.value.size.width,
            height: vc.value.size.height,
            child: VideoPlayer(vc),
          ),
        ),
      );
    }
    if (b == null) {
      final h = widget.height ?? widget.size;
      if (_failed) {
        return widget.fallback ?? SizedBox(width: widget.size, height: h);
      }
      return SizedBox(width: widget.size, height: h);
    }
    final dpr = MediaQuery.devicePixelRatioOf(context);
    return Image.memory(
      b,
      width: widget.size,
      height: widget.height ?? widget.size,
      fit: widget.fit,
      gaplessPlayback: true,
      filterQuality: FilterQuality.low,
      // O'z o'lchamida dekodlanadi — xotira tejaladi.
      cacheWidth: math.max(16, (widget.size * dpr).round()),
      errorBuilder: (_, __, ___) {
        PackService.instance.lastError =
            'Rasm dekodlanmadi (${b.length} bayt, pack ${widget.pack}/${widget.item})';
        return widget.fallback ??
            SizedBox(width: widget.size, height: widget.size);
      },
    );
  }
}

/// Xabar yoki izohdagi stiker / GIF (`pk_<to'plam>_<element>`).
/// Ovozli GIF: bosilsa ovoz yoqiladi/o'chadi; oddiy stiker/GIF: to'plam ochiladi.
class PackMediaView extends StatefulWidget {
  final String file;
  final String type;
  const PackMediaView({super.key, required this.file, required this.type});

  @override
  State<PackMediaView> createState() => _PackMediaViewState();
}

class _PackMediaViewState extends State<PackMediaView> {
  bool get _sound => PackSoundHub.instance.isOn(this);

  @override
  void initState() {
    super.initState();
    PackSoundHub.instance.addListener(_onHub);
    // Sarlavha hali xotirada bo'lmasa — video ekani keyin ma'lum bo'ladi.
    final ref = parsePackRef(widget.file);
    if (ref != null && PackService.instance.cachedItem(ref.$1, ref.$2) == null) {
      unawaited(PackService.instance.resolve(ref.$1, ref.$2).then((_) {
        if (mounted) setState(() {});
      }));
    }
  }

  void _onHub() {
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    PackSoundHub.instance.removeListener(_onHub);
    PackSoundHub.instance.off(this);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ref = parsePackRef(widget.file);
    if (ref == null) return MediaPlaceholder(type: widget.type);
    final size = widget.type == 'gif' ? 200.0 : 150.0;
    final hdr = PackService.instance.cachedItem(ref.$1, ref.$2);
    final video = hdr?.video ?? false;
    return GestureDetector(
      onTap: () {
        if (video) {
          if (_sound) {
            PackSoundHub.instance.off(this);
          } else {
            PackSoundHub.instance.on(this);
          }
          return;
        }
        Navigator.of(context).push(MaterialPageRoute<void>(
            builder: (_) => PackDetailScreen(packId: ref.$1)));
      },
      child: Stack(
        alignment: Alignment.bottomRight,
        children: [
          PackImage(
            pack: ref.$1,
            item: ref.$2,
            size: size,
            animate: true,
            sound: _sound,
            fallback: MediaPlaceholder(type: widget.type),
          ),
          if (video)
            Container(
              margin: const EdgeInsets.all(6),
              padding: const EdgeInsets.all(4),
              decoration: const BoxDecoration(
                  color: Colors.black54, shape: BoxShape.circle),
              child: Icon(
                  _sound ? Icons.volume_up_rounded : Icons.volume_off_rounded,
                  color: Colors.white,
                  size: 16),
            ),
        ],
      ),
    );
  }
}

/// Matn ichidagi maxsus emoji (`[pe:...]`): [size] — matn balandligi.
class PackEmojiInline extends StatelessWidget {
  final int pack;
  final int item;
  final String emoji;
  final double size;
  const PackEmojiInline({
    super.key,
    required this.pack,
    required this.item,
    required this.emoji,
    required this.size,
  });

  @override
  Widget build(BuildContext context) {
    return PackImage(
      pack: pack,
      item: item,
      size: size,
      animate: true,
      // To'plam yuklanmasa oddiy emoji ko'rinadi.
      fallback: SizedBox(
        width: size,
        height: size,
        child: Center(
          child: Text(emoji, style: TextStyle(fontSize: size * 0.8, height: 1)),
        ),
      ),
    );
  }
}

/// `[pe:...]` belgilari bor matnni bo'laklarga ajratadi: oddiy matn
/// [textSpan] orqali, maxsus emoji rasm bo'lib. Belgi bo'lmasa `null`.
List<InlineSpan>? packEmojiSpans(
  String text,
  double fontSize,
  InlineSpan Function(String plain) textSpan,
) {
  if (!text.contains('[pe:')) return null;
  final out = <InlineSpan>[];
  var last = 0;
  for (final m in kPackEmojiToken.allMatches(text)) {
    if (m.start > last) out.add(textSpan(text.substring(last, m.start)));
    out.add(WidgetSpan(
      alignment: PlaceholderAlignment.middle,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 1),
        child: PackEmojiInline(
          pack: int.parse(m.group(1)!),
          item: int.parse(m.group(2)!),
          emoji: m.group(3) ?? '',
          size: fontSize * 1.35,
        ),
      ),
    ));
    last = m.end;
  }
  if (out.isEmpty) return null;
  if (last < text.length) out.add(textSpan(text.substring(last)));
  return out;
}
