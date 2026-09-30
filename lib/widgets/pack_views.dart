// lib/widgets/pack_views.dart — TO'PLAM ELEMENTLARINI KO'RSATISH.
//
// Telefonga bosim tushmasligi uchun (`pack_service.dart` boshidagi izoh):
//
//   * animatsiya FAQAT ekranda to'liq ko'ringan elementda ishlaydi (hamma
//     joyda: panel, chat, izoh, bosib turilgandagi ko'rinish). Qisman
//     ko'rinsa, orqadagi/yopiq sahifada (`TickerMode`) yoki ilova fonda
//     bo'lsa — kichik statik rasm, joyi (`AnimSlots`) bo'shatiladi;
//   * ko'rinish 300 ms da bir tekshiriladi (`_VisWatch`) va aylantirish
//     to'xtaganda darhol — panel ochilib-yopilganda ham kechikmaydi;
//   * `AnimSlots` chegarasi ekranga sig'adigan elementlardan ko'p, ya'ni
//     ko'ringan hammasi o'ynaydi; faqat favqulodda (juda ko'p) holatda
//     telefonni qotirmaslik uchun cheklaydi (`DevicePerf`);
//   * bosib turilgandagi katta ko'rinish (`priority`) joy bo'lmasa
//     boshqasining joyini vaqtincha oladi;
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
  final Set<Object> _prio = {};

  // Faqat to'liq ko'ringanlar joy oladi, shuning uchun chegara bir ekranga
  // sig'adigan elementlardan (emoji paneli ~50, stiker ~15, GIF ~10) ko'p.
  int _cap(AnimPool p) => switch (p) {
        AnimPool.small => switch (DevicePerf.cls) {
            PerfClass.low => 48,
            PerfClass.average => 72,
            PerfClass.high => 120,
          },
        AnimPool.big => switch (DevicePerf.cls) {
            PerfClass.low => 16,
            PerfClass.average => 24,
            PerfClass.high => 36,
          },
        AnimPool.video => switch (DevicePerf.cls) {
            PerfClass.low => 4,
            PerfClass.average => 6,
            PerfClass.high => 8,
          },
      };

  /// [priority] (bosib turilgandagi katta ko'rinish): joy bo'lmasa shu
  /// hovuzdagi eng eski oddiy egasining joyi olinadi — u statik rasmga
  /// qaytib, joy bo'shashini kutadi.
  bool tryAcquire(Object owner,
      [AnimPool pool = AnimPool.big, bool priority = false]) {
    if (_used.containsKey(owner)) return true;
    final n = _used.values.where((v) => v == pool).length;
    if (n >= _cap(pool)) {
      if (!priority) return false;
      Object? victim;
      for (final e in _used.entries) {
        if (e.value == pool && !_prio.contains(e.key) && e.key is AnimSlotOwner) {
          victim = e.key;
          break;
        }
      }
      if (victim == null) return false;
      _used.remove(victim);
      (victim as AnimSlotOwner).slotEvicted();
    }
    _used[owner] = pool;
    if (priority) _prio.add(owner);
    return true;
  }

  /// Joy bo'shadi — kutayotganlar qayta urinadi (animatsiya keyin boshlanadi).
  void release(Object owner) {
    _prio.remove(owner);
    if (_used.remove(owner) != null) notifyListeners();
  }
}

/// [AnimSlots] joyini ushlab turuvchi: joyi ustuvor elementga berilsa xabar oladi.
abstract class AnimSlotOwner {
  void slotEvicted();
}

/// Hamma [PackImage] ko'rinishini davriy tekshiradi: aylantirishsiz o'zgarishlar
/// (panel ochilishi/yopilishi, sahifa almashishi, klaviatura) ham sezilsin.
/// Bitta umumiy taymer — har bir element uchun alohida emas.
class _VisWatch {
  static final Set<_PackImageState> _items = {};
  static Timer? _timer;

  static void add(_PackImageState s) {
    _items.add(s);
    _timer ??= Timer.periodic(const Duration(milliseconds: 300), (_) {
      for (final s in _items.toList()) {
        s._recheck();
      }
    });
  }

  static void remove(_PackImageState s) {
    _items.remove(s);
    if (_items.isEmpty) {
      _timer?.cancel();
      _timer = null;
    }
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

  /// Bosib turilgandagi katta ko'rinish: joy bo'lmasa boshqasinikini oladi.
  final bool priority;

  /// Animatsiya to'xtab statik rasmga qaytdi (ekrandan chiqdi yoki joyi
  /// olindi) — masalan, GIF ovozini o'chirish uchun.
  final VoidCallback? onStopped;

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
    this.priority = false,
    this.onStopped,
  });

  @override
  State<PackImage> createState() => _PackImageState();
}

class _PackImageState extends State<PackImage> implements AnimSlotOwner {
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

  /// To'liq element hozir olinyapti / o'ynatish boshlanyapti.
  bool _fullBusy = false;

  /// To'liq element olinmadi — qayta urinishni taymer ([_scheduleRetry])
  /// yoki element ekrandan chiqib qaytishi boshlaydi (har tekshiruvda emas).
  bool _fullFailed = false;

  /// Sahifa ko'rinyaptimi (`TickerMode`: yopiq panel, orqadagi oyna — yo'q).
  bool _tickerOn = true;
  Size _screen = Size.zero;

  /// Elementning o'zi shu hajmdan katta bo'lsa, kichik rasm o'rniga
  /// yuklanmaydi (devorda bir vaqtda ko'p og'ir fayl olinmasin).
  static const int _fallbackMax = 300 * 1024;

  @override
  void initState() {
    super.initState();
    _VisWatch.add(this);
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
    if (pos.maxScrollExtent <= 0 && pos.minScrollExtent >= 0) return true;
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
    _screen = MediaQuery.sizeOf(context);
    final on = TickerMode.of(context);
    if (on != _tickerOn) {
      _tickerOn = on;
      WidgetsBinding.instance.addPostFrameCallback((_) => _recheck());
    }
    final pos = Scrollable.maybeOf(context)?.position;
    if (!identical(pos, _scrollPos)) {
      _scrollPos?.removeListener(_onScroll);
      _scrollPos = pos;
      pos?.addListener(_onScroll);
    }
  }

  /// Element ekranda TO'LIQ ko'rinyaptimi: ekran ichida va o'zini o'rab
  /// turgan HAMMA aylanuvchi ro'yxatlar (ichma-ich ham) ichida to'liq.
  /// Sahifa yopiq/orqada (`TickerMode`) yoki ilova fonda bo'lsa — yo'q.
  bool _fullyVisible() {
    if (!_tickerOn) return false;
    final ls = WidgetsBinding.instance.lifecycleState;
    if (ls != null && ls != AppLifecycleState.resumed) return false;
    final ro = context.findRenderObject();
    if (ro is! RenderBox || !ro.attached || !ro.hasSize || ro.size.isEmpty) {
      return false;
    }
    try {
      final r = MatrixUtils.transformRect(
          ro.getTransformTo(null), Offset.zero & ro.size);
      const eps = 1.5;
      bool inside(Rect o) =>
          r.left >= o.left - eps &&
          r.top >= o.top - eps &&
          r.right <= o.right + eps &&
          r.bottom <= o.bottom + eps;
      if (_screen != Size.zero && !inside(Offset.zero & _screen)) return false;
      RenderObject? p = ro.parent;
      while (p != null) {
        if (p is RenderBox && p is RenderAbstractViewport && p.hasSize) {
          final vr = MatrixUtils.transformRect(
              p.getTransformTo(null), Offset.zero & p.size);
          if (!inside(vr)) return false;
        }
        p = p.parent;
      }
      return true;
    } catch (_) {
      return false; // hali joylashmagan — keyingi tekshiruvda aniqlanadi
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
    _visTimer = Timer(const Duration(milliseconds: 80), _recheck);
  }

  /// Aylantirish to'xtagach: to'liq ko'ringanlar animatsiyani boshlaydi,
  /// ko'rinmay qolganlar joyini bo'shatib statik rasmga qaytadi.
  void _recheck() {
    if (!mounted) return;
    if (_pendingLoad && _withinWindow()) {
      _pendingLoad = false;
      unawaited(_load());
      return;
    }
    if (!widget.animate) return;
    final r = _ref;
    if (r == null || !r.item.animated) return;
    if (_fullyVisible()) {
      if (!_slot && !_waitingSlot && !_fullBusy && !_fullFailed) {
        unawaited(_loadFull(r, _gen));
      }
    } else {
      // Ekrandan chiqdi — qaytganda muvaffaqiyatsiz element yana sinaladi.
      _fullFailed = false;
      if (_slot || _waitingSlot || _vc != null) _toStatic();
    }
  }

  /// Animatsiya to'xtaydi: joy bo'shaydi, video yopiladi, kichik rasm qoladi.
  void _toStatic() {
    _stopWaiting();
    _release();
    _dropVideo();
    final t = _thumbBytes;
    setState(() {
      if (t != null) _bytes = t;
    });
    widget.onStopped?.call();
  }

  /// Joyimiz ustuvor elementga (bosib turilgan ko'rinish) berildi: statik
  /// rasmga qaytamiz va joy bo'shashini kutamiz.
  @override
  void slotEvicted() {
    _slot = false;
    if (!mounted) return;
    _dropVideo();
    final t = _thumbBytes;
    setState(() {
      if (t != null) _bytes = t;
    });
    if (!_waitingSlot) {
      _waitingSlot = true;
      AnimSlots.instance.addListener(_onSlotFree);
    }
    widget.onStopped?.call();
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
    _VisWatch.remove(this);
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
    _fullFailed = false;
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
      _fullFailed = false;
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

  /// Elementning o'zi. Animatsiya bo'lsa faqat to'liq ko'ringanda va bo'sh
  /// joy ([AnimSlots]) bo'lganda; bo'lmasa kichik statik rasm turadi.
  Future<void> _loadFull(PackRef r, int gen) async {
    if (_fullBusy) return;
    final svc = PackService.instance;
    if (r.item.animated && !_slot) {
      // To'liq ko'rinmasa — `_recheck` ko'ringanda boshlaydi.
      if (!_fullyVisible()) return;
      final pool = r.item.video
          ? AnimPool.video
          : (r.item.len <= 200 * 1024 ? AnimPool.small : AnimPool.big);
      _pool = pool;
      if (!AnimSlots.instance.tryAcquire(this, pool, widget.priority)) {
        if (!_waitingSlot) {
          _waitingSlot = true;
          AnimSlots.instance.addListener(_onSlotFree);
        }
        return;
      }
      _slot = true;
    }
    _fullBusy = true;
    try {
      final d = await svc.data(r);
      if (!mounted || gen != _gen) return;
      // Kutish davomida ekrandan chiqdi yoki joyi olindi.
      if (r.item.animated && !_slot) return;
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
        _fullFailed = true;
        if (_bytes == null) {
          setState(() => _failed = true);
        }
        // Kichik rasm turgan bo'lsa ham to'liq elementni qayta urinib ko'radi.
        _scheduleRetry();
      }
    } finally {
      _fullBusy = false;
    }
  }

  /// Ovozli MP4: vaqtinchalik faylga yoziladi va takrorlanib o'ynaydi.
  Future<void> _startVideo(PackRef r, Uint8List d, int gen) async {
    try {
      final f = File(
          '${Directory.systemTemp.path}/aru_pv_${r.info.id}_${r.item.id}_${identityHashCode(this)}.mp4');
      await f.writeAsBytes(d, flush: true);
      final c = VideoPlayerController.file(f,
          videoPlayerOptions: VideoPlayerOptions(mixWithOthers: true));
      await c.initialize();
      if (!mounted || gen != _gen || !_slot) {
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
      // Dekoder band (juda ko'p video) — statik rasm, keyinroq qayta uriniladi.
      _release();
      _fullFailed = true;
      _scheduleRetry();
    }
  }

  void _onSlotFree() {
    final r = _ref;
    if (!mounted || r == null || !_waitingSlot) return;
    if (!_fullyVisible()) {
      // Ko'rinmay qoldi — ko'ringanda `_recheck` qayta boshlaydi.
      _stopWaiting();
      return;
    }
    if (AnimSlots.instance.tryAcquire(this, _pool, widget.priority)) {
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
            // Ekrandan chiqsa ovoz ham o'chadi.
            onStopped: () => PackSoundHub.instance.off(this),
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
