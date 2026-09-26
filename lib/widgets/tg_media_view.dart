// lib/widgets/tg_media_view.dart — Telegram stikeri, maxsus emoji va
// GIF'ni chizish (`lib/services/tg_media.dart`).
//
//   * `webp` / rasm — oddiy rasm;
//   * `tgs` (Lottie) va `webm` (video stiker, shaffof fon bilan) —
//     Telegram'dagidek ilova ichidagi rlottie va libvpx bilan, fon
//     isolate'ida (`TgAnimView`);
//   * GIF — ovozsiz, takrorlanadigan mp4.

import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:path_provider/path_provider.dart';
import 'package:video_player/video_player.dart';

import '../services/api_base.dart';
import '../services/native_pool.dart';
import '../services/telegram_service.dart';
import '../services/tg_media.dart';

/// Stiker yoki maxsus emoji.
///
/// TALAB (foydalanuvchi): "premium emojining o'rniga oddiy emoji
/// ko'rsatilmasin — faqat premium emojining o'zi yuklab olingach
/// ko'rsatilsin; GIF va stiker ham shunday". Ya'ni yuklanguncha joy
/// BO'SH (xira doira) turadi, zaxira emoji chizilmaydi.
class TgStickerView extends StatelessWidget {
  final TgDoc doc;
  final double size;

  /// Panelda (Telegram'dagidek): kichikroq o'lchamda, 30 kadr/s.
  final bool still;

  /// Faqat birinchi kadr (panel tepasidagi to'plam belgilari).
  final bool frozen;

  const TgStickerView({
    super.key,
    required this.doc,
    required this.size,
    this.still = false,
    this.frozen = false,
  });

  @override
  Widget build(BuildContext context) {
    final animated = doc.kind == 'tgs' || doc.kind == 'webm';
    return SizedBox(
      width: size,
      height: size,
      child: _Deferred(
        key: ValueKey('${doc.id}/${doc.kind}'),
        builder: (context) => FutureBuilder<String?>(
          future:
              TgMedia.instance.file(doc, thumb: doc.kind == 'mp4' && doc.thumb),
          builder: (context, snap) {
            final path = snap.data;
            if (path == null) return _Placeholder(size);
            if (animated) {
              return TgAnimView(
                key: ValueKey(path),
                path: path,
                size: size,
                panel: still,
                frozen: frozen,
                fallback: _Placeholder(size),
              );
            }
            final px = (size * MediaQuery.devicePixelRatioOf(context)).round();
            return Image.file(
              File(path),
              width: size,
              height: size,
              // Asl 512px emas — ko'rinadigan o'lchamda ochiladi.
              cacheWidth: px,
              fit: BoxFit.contain,
              gaplessPlayback: true,
              frameBuilder: (context, child, frame, sync) => AnimatedOpacity(
                opacity: frame == null ? 0 : 1,
                duration: const Duration(milliseconds: 150),
                child: child,
              ),
              errorBuilder: (_, __, ___) => _Placeholder(size),
            );
          },
        ),
      ),
    );
  }
}

/// Yuklanguncha: zaxira emoji EMAS, xira bo'sh joy.
class _Placeholder extends StatelessWidget {
  final double size;
  const _Placeholder(this.size);

  @override
  Widget build(BuildContext context) => Center(
        child: Container(
          width: size * 0.62,
          height: size * 0.62,
          decoration: BoxDecoration(
            color: Colors.white.withValues(alpha: 0.05),
            shape: BoxShape.circle,
          ),
        ),
      );
}

/// Ro'yxat TEZ surilayotganda yuklashni kechiktiradi
/// (`Scrollable.recommendDeferredLoadingForContext`) — barmoq bilan
/// "uchirilgan" ro'yxatdagi yuzlab stiker yuklanib o'tirmaydi, faqat
/// to'xtagan joydagilari yuklanadi.
class _Deferred extends StatefulWidget {
  final WidgetBuilder builder;
  const _Deferred({super.key, required this.builder});

  @override
  State<_Deferred> createState() => _DeferredState();
}

class _DeferredState extends State<_Deferred> {
  bool _go = false;
  Timer? _t;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_go) _check();
  }

  void _check() {
    if (!mounted || _go) return;
    if (Scrollable.recommendDeferredLoadingForContext(context)) {
      _t?.cancel();
      _t = Timer(const Duration(milliseconds: 120), _check);
      return;
    }
    setState(() => _go = true);
  }

  @override
  void dispose() {
    _t?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      _go ? widget.builder(context) : const SizedBox.expand();
}

// ═══════════════════════════════════════════════════════════════
//  ANIMATSIYA (rlottie / libvpx, Rust yadrosida — `sticker_anim.rs`)
// ═══════════════════════════════════════════════════════════════
//
// Telegram/Cherrygram (`RLottieDrawable`) kabi:
//   * kadr FON isolate'ida chiziladi (`NativePool.render`), UI oqimi
//     faqat tayyor rasmni ko'rsatadi;
//   * bitta animatsiyaga bir vaqtda faqat BITTA so'rov: kadr
//     ulgurmasa tashlab o'tiladi;
//   * panelda 30 kadr/s va kichikroq o'lcham;
//   * FAQAT ko'rinadigan (TickerMode yoqilgan) animatsiya harakatlanadi;
//     yashirin sahifadagisi Rust tutqichini ham, kadrlar keshini ham
//     bo'shatadi (oxirgi kadr qoladi) — xotira to'lib ilova yopilmasin;
//   * kadrlar keshi UMUMIY chegara bilan ([_cacheBudget]).
//
// TOPILGAN XATO: ilgari panelda "18 ta harakatlanish o'rni" bor edi va
// ular yashirin sahifadagi (Emoji) to'plam kataklari va tepadagi
// belgilar bilan band bo'lib qolardi — Stikerlar sahifasi umuman
// harakatlanmasdi. Har animatsiya 3 MB gacha kadr saqlardi — ko'p
// stiker ochilganda xotira tugardi.

/// Birinchi kadrlar keshi — panel qayta ochilganda darhol ko'rinsin.
final Map<String, ui.Image> _firstFrames = {};
const _firstFramesMax = 150;

/// Hamma animatsiyalarning kadrlar keshi uchun umumiy chegara.
const _cacheBudget = 56 * 1024 * 1024;
int _cacheUsed = 0;

// ── UMUMIY SOAT ──────────────────────────────────────────────────
//
// TALAB (foydalanuvchi): "emoji, gif va stikerlar judayam sekin
// yuklanyapti, telefonni qotirib, qizdirib yuboryapti — kuchsiz
// telefonlarda ham qotmasdan, kam bosim bilan ishlasin".
//
// TOPILGAN SABAB: har animatsiyaning O'Z soati bor edi va har biri
// o'z kadrini fon isolate'iga so'rardi — ekrandagi 50 ta emoji
// soniyasiga ~1500 ta kadr chizdirardi, protsessorning hamma yadrosi
// to'xtovsiz band, telefon qiziydi, UI oqimi esa har kadrda 50 ta
// `setState` qilardi.
//
// ENDI (Telegram `RLottieDrawable` / `AnimatedEmojiDrawable` kabi):
//   * bitta umumiy soat; hamma animatsiya 30 kadr/s dan oshmaydi
//     (Telegram `limitFps`);
//   * bir vaqtda chiziladigan kadrlar soni CHEKLANGAN (yadrolar
//     soniga qarab 1..3) — navbat bilan, avval hali hech narsa
//     ko'rsatmayotganlari;
//   * chizilgan kadr xotirada qoladi — birinchi aylanishdan keyin
//     animatsiya protsessorni deyarli ishlatmaydi (faqat tayyor
//     rasmni almashtiradi);
//   * ro'yxat SURILAYOTGANDA yangi kadr chizilmaydi (tayyorlari
//     o'ynayveradi) — surish silliq bo'ladi;
//   * kadr `setState` bilan emas, faqat qayta chizish bilan
//     almashadi (widget qayta qurilmaydi).

/// Ro'yxat surilayotganini bildiradi (panel `NotificationListener`
/// orqali chaqiradi).
void tgAnimScrolled() => _AnimClock.instance.lastScroll = DateTime.now();

class _AnimClock {
  _AnimClock._();
  static final instance = _AnimClock._();

  final Set<_TgAnimViewState> _subs = {};

  /// Kadr kutayotganlar (qo'shilish tartibida).
  final Set<_TgAnimViewState> _queue = {};
  int _inflight = 0;
  bool _scheduled = false;
  DateTime lastScroll = DateTime.fromMillisecondsSinceEpoch(0);

  static final int _maxInflight =
      (Platform.numberOfProcessors ~/ 3).clamp(1, 3);

  bool get scrolling =>
      DateTime.now().difference(lastScroll).inMilliseconds < 180;

  void add(_TgAnimViewState s) {
    _subs.add(s);
    _schedule();
  }

  void remove(_TgAnimViewState s) {
    _subs.remove(s);
    _queue.remove(s);
  }

  /// Soat soniyasiga ~30 marta uradi (90/120 Hz ekranda ham har
  /// vsync'da emas) — kadr vsync'ga tekislanadi, lekin ortiqcha
  /// kadr qurilmaydi.
  void _schedule() {
    if (_scheduled || _subs.isEmpty) return;
    _scheduled = true;
    Timer(const Duration(milliseconds: 30), () {
      if (_subs.isEmpty) {
        _scheduled = false;
        return;
      }
      SchedulerBinding.instance.scheduleFrameCallback(_frame);
    });
  }

  void _frame(Duration t) {
    _scheduled = false;
    for (final s in _subs.toList()) {
      s._tick(t);
    }
    _pump();
    _schedule();
  }

  /// Kadr kerak ([first] — hali hech narsa ko'rsatilmagan).
  void want(_TgAnimViewState s, {bool first = false}) {
    if (first) {
      // Bo'sh turganlar navbat boshiga.
      final rest = _queue.toList();
      _queue
        ..clear()
        ..add(s)
        ..addAll(rest);
    } else {
      _queue.add(s);
    }
    _pump();
  }

  void _pump() {
    final busy = scrolling;
    while (_inflight < (busy ? 1 : _maxInflight) && _queue.isNotEmpty) {
      _TgAnimViewState? pick;
      for (final s in _queue) {
        // Surilayotganda faqat birinchi kadrlar chiziladi.
        if (!busy || s._image == null) {
          pick = s;
          break;
        }
      }
      if (pick == null) return;
      _queue.remove(pick);
      _inflight++;
      pick._renderWanted().whenComplete(() {
        _inflight--;
        _pump();
      });
    }
  }
}

class TgAnimView extends StatefulWidget {
  final String path;
  final double size;
  final bool panel;
  final bool frozen;
  final Widget fallback;

  const TgAnimView({
    super.key,
    required this.path,
    required this.size,
    required this.fallback,
    this.panel = false,
    this.frozen = false,
  });

  @override
  State<TgAnimView> createState() => _TgAnimViewState();
}

class _TgAnimViewState extends State<TgAnimView> {
  int _handle = 0;
  bool _opening = false;
  bool _failed = false;
  int _frames = 1;
  double _fps = 30;
  int _px = 0;
  final _img = ValueNotifier<ui.Image?>(null);
  ui.Image? get _image => _img.value;
  bool _dead = false;
  int _shown = -1;

  /// Chizilishi kerak bo'lgan kadr (-1 — hech narsa).
  int _want = -1;
  Duration? _base;
  ValueListenable<TickerModeData>? _tickerMode;
  bool _enabled = true;

  /// Xotiradagi kadrlar (umumiy chegaraga sig'sa).
  Map<int, ui.Image>? _cache;
  int _cacheBytes = 0;

  String get _key => '${widget.path}@$_px';

  /// Ko'rsatiladigan tezlik — 30 kadr/s dan oshmaydi.
  double get _showFps => _fps > 30 ? 30.0 : _fps;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final tm = TickerMode.getValuesNotifier(context);
    if (!identical(tm, _tickerMode)) {
      _tickerMode?.removeListener(_onTickerMode);
      _tickerMode = tm..addListener(_onTickerMode);
      _enabled = tm.value.enabled;
    }
    if (_px != 0) return;
    final dpr = MediaQuery.devicePixelRatioOf(context);
    // Panelda kichik: emoji 100 px, stiker 160 px (Telegram
    // klaviaturasi ham kichraytirib chizadi); xabarda 320 px.
    final cap = widget.panel ? (widget.size <= 48 ? 100 : 160) : 320;
    _px = (widget.size * dpr).round().clamp(24, cap).toInt();
    final first = _firstFrames[_key];
    if (first != null) _img.value = first.clone();
    if (_enabled || _image == null) _open();
  }

  void _onTickerMode() {
    final on = _tickerMode?.value.enabled ?? true;
    if (on == _enabled) return;
    _enabled = on;
    if (on) {
      _open();
    } else {
      _release();
    }
  }

  /// Yashirin: Rust tutqichi va kadrlar keshi bo'shaydi, oxirgi kadr
  /// ekranda qoladi.
  void _release() {
    _AnimClock.instance.remove(this);
    _want = -1;
    _base = null;
    if (_handle > 0) NativePool.render.animClose(_handle);
    _handle = 0;
    _dropCache();
  }

  void _dropCache() {
    for (final i in _cache?.values ?? const <ui.Image>[]) {
      i.dispose();
    }
    _cache = null;
    _cacheUsed -= _cacheBytes;
    _cacheBytes = 0;
  }

  Future<void> _open() async {
    if (_opening || _handle > 0 || _failed || _dead) return;
    _opening = true;
    final r = await NativePool.render.animOpen(widget.path, _px, _px);
    _opening = false;
    if (_dead || !_enabled && _image != null) {
      if (r != null) NativePool.render.animClose(r.$1);
      return;
    }
    if (r == null) {
      setState(() => _failed = true);
      return;
    }
    _handle = r.$1;
    _frames = r.$2.clamp(1, 100000);
    _fps = r.$3 > 0 ? r.$3 : 30;
    // Faqat KO'RSATILADIGAN kadrlar saqlanadi (60 -> 30 kadr/s da
    // har ikkinchisi).
    final shown = (_frames * _showFps / _fps).ceil();
    final need = _px * _px * 4 * shown;
    if (_cacheUsed + need <= _cacheBudget) {
      _cache = {};
      _cacheBytes = need;
      _cacheUsed += need;
    }
    if (_image == null || _shown < 0) {
      _want = 0;
      _AnimClock.instance.want(this, first: _image == null);
    }
    if (_frames > 1 && !widget.frozen && _enabled) {
      _AnimClock.instance.add(this);
    }
  }

  void _tick(Duration t) {
    if (_handle <= 0) return;
    final base = _base ??= t;
    final fps = _showFps;
    final step = _fps / fps;
    final n = ((t - base).inMicroseconds / 1e6 * fps).floor();
    final f = ((n * step).floor()) % _frames;
    if (f == _shown || f == _want) return;
    final cached = _cache?[f];
    if (cached != null) {
      _show(cached.clone(), f);
      return;
    }
    // Oldingi kadr hali chizilmoqda — bu kadr tashlab o'tiladi.
    if (_want >= 0) return;
    _want = f;
    _AnimClock.instance.want(this);
  }

  /// Soat navbati kelganda chaqiradi.
  Future<void> _renderWanted() async {
    final f = _want;
    final h = _handle;
    if (f < 0 || h <= 0 || _dead) {
      _want = -1;
      return;
    }
    final bytes = await NativePool.render.animFrame(h, f, _px, _px);
    if (_dead || bytes == null || h != _handle) {
      _want = -1;
      return;
    }
    final c = Completer<ui.Image>();
    ui.decodeImageFromPixels(
        bytes, _px, _px, ui.PixelFormat.rgba8888, c.complete);
    final img = await c.future;
    _want = -1;
    if (_dead) {
      img.dispose();
      return;
    }
    if (f == 0 && !_firstFrames.containsKey(_key)) {
      if (_firstFrames.length >= _firstFramesMax) {
        final k = _firstFrames.keys.first;
        _firstFrames.remove(k)?.dispose();
      }
      _firstFrames[_key] = img.clone();
    }
    final cache = _cache;
    if (cache != null && h == _handle && !cache.containsKey(f)) {
      cache[f] = img.clone();
    }
    _show(img, f);
  }

  void _show(ui.Image img, int f) {
    _shown = f;
    final old = _img.value;
    _img.value = img;
    old?.dispose();
    // Birinchi kadr — `fallback`/bo'sh joy o'rniga rasm chiqsin.
    if (old == null && mounted) setState(() {});
  }

  @override
  void dispose() {
    _dead = true;
    _tickerMode?.removeListener(_onTickerMode);
    _release();
    _img.value?.dispose();
    _img.value = null;
    _img.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_image == null) {
      return _failed ? widget.fallback : const SizedBox.shrink();
    }
    return CustomPaint(
      size: Size.square(widget.size),
      painter: _FramePainter(_img),
    );
  }
}

/// Kadrni chizadi; kadr almashganda faqat QAYTA CHIZILADI (qayta
/// qurilmaydi, joylanmaydi).
class _FramePainter extends CustomPainter {
  final ValueNotifier<ui.Image?> img;
  _FramePainter(this.img) : super(repaint: img);

  static final _paint = Paint()..filterQuality = FilterQuality.medium;

  @override
  void paint(Canvas canvas, Size size) {
    final i = img.value;
    if (i == null) return;
    final side = size.shortestSide;
    final dst = Rect.fromCenter(
        center: size.center(Offset.zero), width: side, height: side);
    canvas.drawImageRect(
        i,
        Rect.fromLTWH(0, 0, i.width.toDouble(), i.height.toDouble()),
        dst,
        _paint);
  }

  @override
  bool shouldRepaint(_FramePainter old) => !identical(old.img, img);
}

/// Xabardagi stiker (`stk_...` havolasi bo'yicha).
class TgStickerRefView extends StatelessWidget {
  final String ref;
  final double size;
  const TgStickerRefView({super.key, required this.ref, this.size = 150});

  @override
  Widget build(BuildContext context) {
    // TOPILGAN XATO ("izohga boshqa stiker ketyapti"): ro'yxatga yangi
    // izoh qo'shilganda eski katak boshqa izohga qayta ishlatilardi,
    // `FutureBuilder` esa yangi javob kelguncha ESKI stikerni, ichidagi
    // animatsiya esa fayl almashganini sezmay eski stikerni ko'rsatib
    // qolardi. Endi kalit havolaga bog'langan — katak butunlay yangilanadi.
    return FutureBuilder<TgDoc?>(
      key: ValueKey(ref),
      future: TgMedia.instance.stickerByRef(ref),
      builder: (context, snap) {
        final d = snap.data;
        if (d == null) {
          return SizedBox(
            width: size,
            height: size,
            child: snap.connectionState == ConnectionState.done
                ? Icon(Icons.image_not_supported_outlined,
                    color: Colors.white.withValues(alpha: 0.25))
                : null,
          );
        }
        return TgStickerView(doc: d, size: size);
      },
    );
  }
}

/// Matn ichidagi maxsus emoji (topilguncha bo'sh joy).
class TgCustomEmojiView extends StatelessWidget {
  final String id;
  final String alt;
  final double size;
  const TgCustomEmojiView(
      {super.key, required this.id, required this.alt, required this.size});

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<TgDoc?>(
      key: ValueKey(id),
      future: TgMedia.instance.customEmoji(id),
      builder: (context, snap) {
        final d = snap.data;
        // Yuklanguncha zaxira emoji EMAS — bo'sh joy (foydalanuvchi talabi).
        if (d == null) return SizedBox(width: size, height: size);
        return TgStickerView(doc: d, size: size);
      },
    );
  }
}

/// Matnni maxsus emoji belgilari (`[ce:id:emoji]`) bilan bo'laklarga
/// ajratadi. Belgi bo'lmasa `null`.
List<InlineSpan>? customEmojiSpans(
    String text, TextStyle style, List<InlineSpan> Function(String) plain) {
  if (!text.contains('[ce:')) return null;
  final out = <InlineSpan>[];
  var last = 0;
  final size = (style.fontSize ?? 14) * 1.35;
  for (final m in customEmojiToken.allMatches(text)) {
    if (m.start > last) out.addAll(plain(text.substring(last, m.start)));
    out.add(WidgetSpan(
      alignment: PlaceholderAlignment.middle,
      child: TgCustomEmojiView(id: m.group(1)!, alt: m.group(2)!, size: size),
    ));
    last = m.end;
  }
  if (out.isEmpty) return null;
  if (last < text.length) out.addAll(plain(text.substring(last)));
  return out;
}

/// Maxsus emoji belgilarini oddiy emoji bilan almashtiradi
/// (bildirishnoma, ro'yxatdagi qisqa matn va nusxa olish uchun).
String plainEmojiText(String text) =>
    text.replaceAllMapped(customEmojiToken, (m) => m.group(2)!);

// ═══════════════════════════════════════════════════════════════
//  GIF
// ═══════════════════════════════════════════════════════════════

/// Panel katagi: GIF.
///
/// TALAB (foydalanuvchi): "GIF'lar to'liq yuklab olinmayapti". Ilgari
/// panelda faqat kichik rasm (birinchi kadr) turardi. Endi Telegram'dagidek:
/// avval kichik rasm, so'ng — katak EKRANDA turgan bo'lsa — GIF'ning
/// o'zi yuklanadi va ovozsiz, takrorlanib o'ynaydi. Bir vaqtda eng
/// ko'pi [_gifSlots] ta o'ynaydi (telefon video dekoderlari cheklangan),
/// qolganlari kichik rasmda turadi; yashirin sahifada hammasi to'xtaydi.
class TgGifThumb extends StatefulWidget {
  final TgDoc doc;
  const TgGifThumb({super.key, required this.doc});

  @override
  State<TgGifThumb> createState() => _TgGifThumbState();
}

/// Kuchsiz (kam yadroli) telefonda bir vaqtda 2 ta, aks holda 4 ta.
final _gifSlots = Platform.numberOfProcessors >= 8 ? 4 : 2;
int _gifBusy = 0;
final List<VoidCallback> _gifWaiters = [];

class _TgGifThumbState extends State<TgGifThumb> {
  VideoPlayerController? _c;
  bool _slot = false;
  bool _dead = false;
  ValueListenable<TickerModeData>? _tm;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final tm = TickerMode.getValuesNotifier(context);
    if (!identical(tm, _tm)) {
      _tm?.removeListener(_onTm);
      _tm = tm..addListener(_onTm);
    }
    if (tm.value.enabled && !_slot) _want();
  }

  void _onTm() {
    if (_tm?.value.enabled ?? false) {
      _want();
    } else {
      _stop();
    }
  }

  void _want() {
    if (_slot || _dead) return;
    if (_gifBusy < _gifSlots) {
      _gifBusy++;
      _slot = true;
      _play();
    } else if (!_gifWaiters.contains(_onSlot)) {
      _gifWaiters.add(_onSlot);
    }
  }

  void _onSlot() {
    if (_dead || _slot) return;
    if (!(_tm?.value.enabled ?? true)) return;
    _gifBusy++;
    _slot = true;
    _play();
  }

  void _freeSlot() {
    _gifWaiters.remove(_onSlot);
    if (!_slot) return;
    _slot = false;
    _gifBusy--;
    while (_gifWaiters.isNotEmpty && _gifBusy < _gifSlots) {
      _gifWaiters.removeAt(0)();
    }
  }

  Future<void> _play() async {
    // Tez surilayotgan ro'yxatda yuklanmaydi.
    // Surish to'xtaguncha video dekoder ochilmaydi (surish silliq).
    while (mounted &&
        (_AnimClock.instance.scrolling ||
            Scrollable.recommendDeferredLoadingForContext(context))) {
      await Future<void>.delayed(const Duration(milliseconds: 150));
    }
    if (!mounted || !_slot) return;
    final path = await TgMedia.instance.file(widget.doc);
    if (!mounted || !_slot || path == null) {
      if (mounted) _freeSlot();
      return;
    }
    final c = VideoPlayerController.file(File(path),
        videoPlayerOptions: VideoPlayerOptions(mixWithOthers: true));
    try {
      await c.initialize();
      await c.setLooping(true);
      await c.setVolume(0);
      await c.play();
    } catch (_) {
      await c.dispose();
      if (mounted) _freeSlot();
      return;
    }
    if (!mounted || !_slot) {
      await c.dispose();
      return;
    }
    setState(() => _c = c);
  }

  void _stop() {
    final c = _c;
    _c = null;
    c?.dispose();
    _freeSlot();
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    _dead = true;
    _tm?.removeListener(_onTm);
    _c?.dispose();
    _c = null;
    _freeSlot();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = _c;
    final dpr = MediaQuery.devicePixelRatioOf(context);
    return Container(
      color: Colors.white.withValues(alpha: 0.05),
      child: Stack(
        fit: StackFit.expand,
        children: [
          if (widget.doc.thumb)
            FutureBuilder<String?>(
              key: ValueKey(widget.doc.id),
              future: TgMedia.instance.file(widget.doc, thumb: true),
              builder: (context, snap) {
                final p = snap.data;
                if (p == null) return const SizedBox();
                return Image.file(File(p),
                    fit: BoxFit.cover,
                    cacheWidth: (200 * dpr).round(),
                    gaplessPlayback: true);
              },
            ),
          if (c != null && c.value.isInitialized)
            FittedBox(
              fit: BoxFit.cover,
              clipBehavior: Clip.hardEdge,
              child: SizedBox(
                width: c.value.size.width,
                height: c.value.size.height,
                child: VideoPlayer(c),
              ),
            ),
        ],
      ),
    );
  }
}

/// Xabardagi GIF: kanaldagi fayl ([fileName]) — ovozsiz, takrorlanib.
class TgGifMessage extends StatefulWidget {
  final String fileName;
  final double maxWidth;
  const TgGifMessage(
      {super.key, required this.fileName, this.maxWidth = 230});

  @override
  State<TgGifMessage> createState() => _TgGifMessageState();
}

final Map<String, Future<File?>> _chatFiles = {};

/// Kanaldagi kichik fayl (GIF, dumaloq video) — bir marta olinadi va
/// vaqtinchalik papkada saqlanadi.
Future<File?> tgChatFile(String name) async {
  final f = await (_chatFiles[name] ??= _fetchChatFile(name));
  // Xotira oynasida kesh tozalangan bo'lsa — qayta olinadi.
  if (f != null && !f.existsSync()) {
    _chatFiles.remove(name);
    return _chatFiles[name] ??= _fetchChatFile(name);
  }
  return f;
}

Future<File?> _fetchChatFile(String name) => () async {
        try {
          final dir = await getTemporaryDirectory();
          final f = File('${dir.path}/gif_$name');
          if (await f.exists() && await f.length() > 0) return f;
          final bytes = await TelegramService.instance
              .fetchBytes('$kApiBase/api/image/$name');
          if (bytes == null || bytes.isEmpty) {
            _chatFiles.remove(name);
            return null;
          }
          await f.writeAsBytes(bytes, flush: true);
          return f;
        } catch (_) {
          _chatFiles.remove(name);
          return null;
        }
      }();


class _TgGifMessageState extends State<TgGifMessage> {
  VideoPlayerController? _ctrl;
  bool _failed = false;

  @override
  void initState() {
    super.initState();
    _open();
  }

  int _tries = 0;

  @override
  void didUpdateWidget(TgGifMessage old) {
    super.didUpdateWidget(old);
    // Ro'yxatdagi katak boshqa xabarga qayta ishlatildi.
    if (old.fileName != widget.fileName) {
      _ctrl?.dispose();
      _ctrl = null;
      _failed = false;
      _tries = 0;
      _open();
    }
  }

  Future<void> _open() async {
    final name = widget.fileName;
    final f = await tgChatFile(name);
    if (!mounted || name != widget.fileName) return;
    if (f == null) {
      // Yangi yuborilgan GIF'ni bot kanalga hali ko'chirmagan bo'lishi
      // mumkin — bir necha marta qayta uriniladi.
      if (_tries++ < 6) {
        await Future<void>.delayed(Duration(seconds: 2 + _tries * 2));
        if (mounted && name == widget.fileName) return _open();
        return;
      }
      setState(() => _failed = true);
      return;
    }
    final c = VideoPlayerController.file(f,
        videoPlayerOptions: VideoPlayerOptions(mixWithOthers: true));
    try {
      await c.initialize();
      await c.setLooping(true);
      await c.setVolume(0);
      await c.play();
    } catch (_) {
      await c.dispose();
      if (mounted) setState(() => _failed = true);
      return;
    }
    if (!mounted) {
      await c.dispose();
      return;
    }
    setState(() => _ctrl = c);
  }

  @override
  void dispose() {
    _ctrl?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = _ctrl;
    final ratio = c != null && c.value.isInitialized && c.value.aspectRatio > 0
        ? c.value.aspectRatio
        : 1.4;
    return ClipRRect(
      borderRadius: BorderRadius.circular(14),
      child: SizedBox(
        width: widget.maxWidth,
        height: widget.maxWidth / ratio,
        child: c == null
            ? Container(
                color: Colors.white.withValues(alpha: 0.06),
                alignment: Alignment.center,
                child: _failed
                    ? Icon(Icons.gif_box_outlined,
                        color: Colors.white.withValues(alpha: 0.3), size: 36)
                    : const SizedBox(
                        width: 22,
                        height: 22,
                        child: CircularProgressIndicator(
                            strokeWidth: 2, color: Colors.white54),
                      ),
              )
            : VideoPlayer(c),
      ),
    );
  }
}
