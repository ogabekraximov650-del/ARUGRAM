// lib/widgets/tg_media_view.dart — Telegram stikeri, maxsus emoji va
// GIF'ni chizish (`lib/services/tg_media.dart`).
//
//   * `webp` / rasm — oddiy rasm;
//   * `tgs` (Lottie) va `webm` (video stiker, shaffof fon bilan) —
//     Telegram'dagidek ilova ichidagi rlottie va libvpx bilan, fon
//     isolate'ida (`TgAnimView`);
//   * GIF — ovozsiz, takrorlanadigan mp4.

import 'dart:async';
import 'dart:math' as math;
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/scheduler.dart';

import '../services/device_perf.dart';
import '../services/native_pool.dart';
import '../services/telegram_service.dart';
import '../services/tg_media.dart';
import 'tg_media_preview.dart';

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

  /// Bir marta o'ynaydi (chatdagi xabar) — [TgAnimView.once].
  final bool once;

  /// Harakatlanadi — faqat katta ko'rinishda ([TgAnimView.live]).
  final bool live;

  const TgStickerView({
    super.key,
    required this.doc,
    required this.size,
    this.still = false,
    this.frozen = false,
    this.once = false,
    this.live = false,
  });

  @override
  Widget build(BuildContext context) {
    final thumb = doc.kind == 'mp4' && doc.thumb;
    // Fayl diskda tayyor — kutish ham, kechiktirish ham yo'q: birinchi
    // kadrdanoq chiziladi.
    final ready = TgMedia.instance.fileSync(doc, thumb: thumb);
    return SizedBox(
      width: size,
      height: size,
      child: ready != null
          ? KeyedSubtree(
              key: ValueKey('${doc.id}/${doc.kind}'),
              child: _content(context, ready))
          : _Deferred(
              key: ValueKey('${doc.id}/${doc.kind}'),
              builder: (context) => FutureBuilder<String?>(
                future: TgMedia.instance.file(doc, thumb: thumb),
                builder: (context, snap) {
                  final path = snap.data;
                  if (path == null) return _Placeholder(size);
                  return _content(context, path);
                },
              ),
            ),
    );
  }

  Widget _content(BuildContext context, String path) {
            final animated = doc.kind == 'tgs' || doc.kind == 'webm';
            if (animated) {
              return TgAnimView(
                key: ValueKey(path),
                path: path,
                size: size,
                panel: still,
                frozen: frozen,
                once: once,
                live: live,
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
/// Kuchsiz telefonda kamroq (har biri ~100-250 KB piksel).
final _firstFramesMax = DevicePerf.low ? 60 : 150;

/// Hamma animatsiyalarning kadrlar keshi uchun umumiy chegara.
/// Telefon kuchiga qarab (`DevicePerf`): 16 / 48 / 64 MB.
final _cacheBudget = switch (DevicePerf.cls) {
      PerfClass.low => 16,
      PerfClass.average => 48,
      PerfClass.high => 64,
    } *
    1024 *
    1024;
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
//     soniga qarab 1..4) — navbat bilan, avval hali hech narsa
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

  static final int _maxInflight = switch (DevicePerf.cls) {
    PerfClass.low => 1,
    PerfClass.average => (Platform.numberOfProcessors ~/ 2).clamp(1, 2),
    PerfClass.high => (Platform.numberOfProcessors ~/ 2).clamp(2, 4),
  };

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

  /// Eni (logik). Bo'yi [height] (berilmasa — kvadrat).
  final double size;
  final double? height;
  final bool panel;
  final bool frozen;

  /// GIF (H.264 MP4): to'rtburchak, "cover" bilan kesiladi.
  final bool gif;

  /// Bir marta o'ynaydi (chatdagi stiker/GIF, Telegram kabi): ekranda
  /// to'liq ko'ringanda boshlanadi, oxirgi kadrda to'xtaydi; ekrandan
  /// chiqib qaytsa (yoki bosilsa) yana bir marta.
  final bool once;

  /// Harakatlanadi. TALAB (foydalanuvchi): "emoji, GIF va stikerlar
  /// oddiy holatda umuman animatsiyalanmasin — faqat ustiga bosib
  /// turganda (yoki chatda bir bosilganda) tepada ko'rsatilganda".
  /// Qolgan hamma joyda — faqat birinchi kadr (Rust tutqichi darhol
  /// yopiladi, yuza ham, soat ham ishlamaydi).
  final bool live;
  final Widget fallback;

  const TgAnimView({
    super.key,
    required this.path,
    required this.size,
    required this.fallback,
    this.height,
    this.panel = false,
    this.frozen = false,
    this.gif = false,
    this.once = false,
    this.live = false,
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
  int _ph = 0;
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

  String get _key => '${widget.path}@${_px}x$_ph';

  /// Faqat birinchi kadr ([TgAnimView.live] emas).
  bool get _still => widget.frozen || !widget.live;

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
    // SIFAT (foydalanuvchi: "GIF va stikerlar sifati pasayib
    // ketibdi"): ilgari xabardagi stiker 320 px, GIF 360 px bilan
    // cheklangan edi — 3x ekranda (180 dp = 540 px) xira ko'rinardi.
    // Endi Telegram kabi ekran o'lchamida (512 px gacha); kadrlar
    // diskda saqlangani uchun qayta chizish qimmat emas.
    final low = DevicePerf.low;
    final cap = widget.panel
        ? (widget.size <= 48 ? (low ? 96 : 128) : (low ? 192 : 256))
        : (low ? 384 : 512);
    if (widget.gif) {
      // GIF: eni 320 (panel) / 512 (xabar) px gacha, bo'yi nisbatda.
      final h = widget.height ?? widget.size;
      final gcap = widget.panel ? (low ? 240 : 320) : (low ? 400 : 512);
      final k = math.min(1.0, gcap / (math.max(widget.size, h) * dpr));
      _px = (widget.size * dpr * k).round().clamp(16, 512).toInt();
      _ph = (h * dpr * k).round().clamp(16, 512).toInt();
    } else {
      _px = (widget.size * dpr).round().clamp(24, cap).toInt();
      _ph = _px;
    }
    // Guruh ichida (emoji qatori va h.k.) — o'z yuzasi YO'Q: o'rnini
    // guruhga aytadi, guruh hammasini bitta yuzaga chizdiradi.
    final batch = _texSupported && !_still ? TgAnimBatch._of(context) : null;
    if (batch != null) {
      _batch = batch;
      _texMode = true;
      _batched = true;
      WidgetsBinding.instance.addPostFrameCallback((_) => _report());
      return;
    }
    // Android: kadrni Rust o'zi Flutter `Texture` ga chizadi (Dart
    // kadrlar bilan shug'ullanmaydi — UI oqimi bo'sh, silliq).
    if (_texSupported && !_still) {
      unawaited(_openTex());
      return;
    }
    final first = _firstFrames[_key];
    if (first != null) _img.value = first.clone();
    if (_enabled || _image == null) _open();
  }

  void _onTickerMode() {
    final on = _tickerMode?.value.enabled ?? true;
    if (on == _enabled) return;
    _enabled = on;
    if (_batched) return;
    if (_texMode) {
      _texPlay();
      return;
    }
    if (on) {
      _open();
    } else {
      _release();
    }
  }

  // ── TEXTURE REJIMI (`rust/src/anim_player.rs`) ─────────────────
  //
  // Birinchi xatoda (eski yig'ma, platforma qo'llamaydi) butun ilova
  // uchun eski yo'lga (Dart orqali) qaytiladi.
  static bool _texBroken = false;
  static const _texCh = MethodChannel('aru/anim');
  bool get _texSupported => Platform.isAndroid && !_texBroken;
  bool _texMode = false;
  _TgAnimBatchState? _batch;
  bool _batched = false;

  /// O'rnini (guruhga nisbatan) guruhga aytadi.
  void _report() {
    final b = _batch;
    if (_dead || b == null || !mounted) return;
    final me = context.findRenderObject() as RenderBox?;
    final root = b.context.findRenderObject() as RenderBox?;
    if (me == null || root == null || !me.hasSize || !me.attached) return;
    final o = me.localToGlobal(Offset.zero, ancestor: root);
    b.put(this, widget.path, o & me.size);
  }

  /// Guruh yuzasi ishlamadi — o'zi chizadi (eski yo'l).
  void _batchFailed() {
    _batch = null;
    _batched = false;
    _texMode = false;
    if (_dead) return;
    final first = _firstFrames[_key];
    if (first != null) _img.value = first.clone();
    _open();
    if (mounted) setState(() {});
  }

  int _player = 0;
  int? _texture;
  bool _texReady = false;
  Timer? _texPoll;

  Future<void> _openTex() async {
    if (_player != 0 || _dead) return;
    _texMode = true;
    final j = await NativePool.render.call('rust_player_open',
        arg: jsonEncode(
            {'path': widget.path, 'w': _px, 'h': _ph, 'once': widget.once}));
    final id = (j['id'] as num?)?.toInt() ?? 0;
    if (id <= 0) {
      if (_dead) return;
      setState(() => _failed = true);
      return;
    }
    if (_dead) {
      AnimPlayers.free(id);
      return;
    }
    _player = id;
    int? tex;
    try {
      tex = await _texCh.invokeMethod<int>(
          'create', {'player': id, 'w': _px, 'h': _ph});
    } catch (_) {
      tex = null;
    }
    if (tex == null) {
      // Texture yo'li ishlamadi — Dart orqali (eski yo'l).
      _texBroken = true;
      AnimPlayers.free(id);
      _player = 0;
      _texMode = false;
      if (!_dead) _open();
      return;
    }
    if (_dead) {
      unawaited(_texCh.invokeMethod('dispose', {'texture': tex})
          .whenComplete(() => AnimPlayers.free(id)));
      return;
    }
    _texture = tex;
    _texPlay();
    if (widget.once) {
      (_watch ??= _OnceWatch(context, () => AnimPlayers.replay(_player)))
          .attach(fresh: true);
    }
    // Birinchi kadr yuzaga chiqqach ko'rsatiladi (bo'sh/qora yuza
    // miltillamasin).
    var waited = 0;
    _texPoll = Timer.periodic(const Duration(milliseconds: 16), (t) {
      waited += 16;
      if (_dead) {
        t.cancel();
        return;
      }
      if (AnimPlayers.drawn(id) || waited > 3000) {
        t.cancel();
        if (mounted) setState(() => _texReady = true);
      }
    });
  }

  void _texPlay() {
    if (_player == 0) return;
    AnimPlayers.setPlaying(_player, _enabled && !_still);
  }

  _OnceWatch? _watch;

  /// Bosilganda — bir martalik animatsiya qayta o'ynaydi.
  void replay() {
    if (_player != 0) AnimPlayers.replay(_player);
    _batch?.replay();
  }

  void _closeTex() {
    _watch?.detach();
    _texPoll?.cancel();
    final id = _player;
    final tex = _texture;
    _player = 0;
    _texture = null;
    if (id == 0) return;
    AnimPlayers.setPlaying(id, false);
    if (tex == null) {
      AnimPlayers.free(id);
    } else {
      unawaited(_texCh.invokeMethod('dispose', {'texture': tex})
          .catchError((_) => null)
          .whenComplete(() => AnimPlayers.free(id)));
    }
  }

  /// Yashirin: Rust tutqichi va kadrlar keshi bo'shaydi, oxirgi kadr
  /// ekranda qoladi.
  void _release() {
    _AnimClock.instance.remove(this);
    _want = -1;
    _base = null;
    _shownN = -1;
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
    // Birinchi kadr allaqachon bor — qayta ochish shart emas.
    if (_still && _image != null) return;
    _opening = true;
    final r = await NativePool.render.animOpen(widget.path, _px, _ph);
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
    final need = _px * _ph * 4 * shown;
    if (!_still && _cacheUsed + need <= _cacheBudget) {
      _cache = {};
      _cacheBytes = need;
      _cacheUsed += need;
    }
    if (_image == null || _shown < 0) {
      _want = 0;
      _wantN = 0;
      _AnimClock.instance.want(this, first: _image == null);
    }
    // TALAB (foydalanuvchi): "emojilar — ekranda ko'rinib turgan
    // barchasi animatsiyalansin" (kuchsiz telefonda ham).
    if (_frames > 1 && !_still && _enabled) {
      _AnimClock.instance.add(this);
    }
  }

  /// Ko'rsatilgan kadrning tartib raqami (vaqt bo'yicha).
  int _shownN = -1;
  int _wantN = -1;

  int _frameOf(int n) => ((n * (_fps / _showFps)).floor()) % _frames;

  void _tick(Duration t) {
    if (_handle <= 0) return;
    final base = _base ??= t;
    final fps = _showFps;
    var n = ((t - base).inMicroseconds / 1e6 * fps).floor();
    var f = _frameOf(n);
    if (f == _shown || f == _want) return;
    final cached = _cache?[f];
    if (cached != null) {
      _show(cached.clone(), f, n);
      return;
    }
    // Oldingi kadr hali chizilmoqda — kutiladi.
    if (_want >= 0) return;
    // TOPILGAN XATO ("stiker qotib yoki 2x tezlikda o'ynayapti"): kadr
    // chizish ulgurmaganda vaqt bo'yicha oldinga SAKRALARDI — kadrlar
    // tashlab ketilib, stiker tez va uzuq-uzuq ko'rinardi. Endi
    // KETMA-KET keyingi kadr chiziladi, soat esa unga moslanadi:
    // kuchsiz telefonda sal sekinroq, lekin silliq.
    if (_shownN >= 0 && n > _shownN + 1) {
      n = _shownN + 1;
      _base = t - Duration(microseconds: (n * 1e6 / fps).round());
      f = _frameOf(n);
      final c2 = _cache?[f];
      if (c2 != null) {
        _show(c2.clone(), f, n);
        return;
      }
    }
    _want = f;
    _wantN = n;
    _AnimClock.instance.want(this);
  }

  /// Soat navbati kelganda chaqiradi.
  Future<void> _renderWanted() async {
    final f = _want;
    final wn = _wantN;
    final h = _handle;
    if (f < 0 || h <= 0 || _dead) {
      _want = -1;
      return;
    }
    final bytes = await NativePool.render.animFrame(h, f, _px, _ph);
    if (_dead || bytes == null || h != _handle) {
      _want = -1;
      return;
    }
    final c = Completer<ui.Image>();
    ui.decodeImageFromPixels(
        bytes, _px, _ph, ui.PixelFormat.rgba8888, c.complete);
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
    _show(img, f, wn);
    // Faqat birinchi kadr kerak (panel stikerlari) — Rust tutqichi
    // darhol yopiladi (xotira: har stiker o'z chizgichini ushlab
    // turmasin).
    if (_still) _release();
  }

  void _show(ui.Image img, int f, [int n = -1]) {
    _shown = f;
    _shownN = n;
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
    _batch?.remove(this);
    _batch = null;
    _closeTex();
    _release();
    _img.value?.dispose();
    _img.value = null;
    _img.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_batched) {
      // Guruh chizadi — bu yerda faqat joy (o'lcham o'zgarsa qayta aytiladi).
      WidgetsBinding.instance.addPostFrameCallback((_) => _report());
      final box =
          SizedBox(width: widget.size, height: widget.height ?? widget.size);
      return widget.once ? _TapReplay(onTap: replay, child: box) : box;
    }
    if (_texMode) {
      final tex = _texture;
      if (_failed) return widget.fallback;
      final box = SizedBox(
        width: widget.size,
        height: widget.height ?? widget.size,
        child: tex != null && _texReady
            ? Texture(textureId: tex, filterQuality: FilterQuality.medium)
            : null,
      );
      return widget.once ? _TapReplay(onTap: replay, child: box) : box;
    }
    if (_image == null) {
      return _failed ? widget.fallback : const SizedBox.shrink();
    }
    return CustomPaint(
      size: Size(widget.size, widget.height ?? widget.size),
      painter: _FramePainter(_img, fill: widget.gif),
    );
  }
}

/// Kadrni chizadi; kadr almashganda faqat QAYTA CHIZILADI (qayta
/// qurilmaydi, joylanmaydi).
class _FramePainter extends CustomPainter {
  final ValueNotifier<ui.Image?> img;

  /// Butun maydonni to'ldiradi (GIF); aks holda markazdagi kvadrat.
  final bool fill;
  _FramePainter(this.img, {this.fill = false}) : super(repaint: img);

  static final _paint = Paint()..filterQuality = FilterQuality.medium;

  @override
  void paint(Canvas canvas, Size size) {
    final i = img.value;
    if (i == null) return;
    final side = size.shortestSide;
    final dst = fill
        ? Offset.zero & size
        : Rect.fromCenter(
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
    final known = TgMedia.instance.stickerByRefSync(ref);
    if (known != null) {
      return KeyedSubtree(key: ValueKey(ref), child: _tapPreview(context, known));
    }
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
        return _tapPreview(context, d);
      },
    );
  }

  /// Turadi (birinchi kadr); bosilsa — tepada katta bo'lib harakatlanadi.
  Widget _tapPreview(BuildContext context, TgDoc d) => GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => TgHoldPreview.showTap(context, doc: d),
        child: TgStickerView(doc: d, size: size),
      );
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
    final known = TgMedia.instance.customEmojiSync(id);
    if (known != null) {
      return KeyedSubtree(
          key: ValueKey(id), child: TgStickerView(doc: known, size: size));
    }
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

/// Panel katagi / ko'rish oynasi: GIF.
///
/// Telegram (`AnimatedFileDrawable`) kabi: GIF telefonning video
/// pleyeri bilan EMAS, ilova ichidagi ffmpeg H.264 dekoderi bilan fonda
/// ochiladi va stikerlar bilan bir xil dvigatelda (umumiy soat, kadrlar
/// diskda) o'ynaydi — dekoderlar soni cheklovi ham, qorayib qolish ham
/// yo'q. Fayl kelguncha — kichik rasm.
class TgGifThumb extends StatelessWidget {
  final TgDoc doc;

  /// `false` — faqat kichik rasm.
  final bool play;

  /// Ko'rish oynasida (kattaroq o'lchamda chiziladi).
  final bool loop;

  const TgGifThumb(
      {super.key, required this.doc, this.play = true, this.loop = false});

  @override
  Widget build(BuildContext context) {
    final dpr = MediaQuery.devicePixelRatioOf(context);
    return LayoutBuilder(builder: (context, box) {
      final w = box.maxWidth.isFinite ? box.maxWidth : 120.0;
      final h = box.maxHeight.isFinite ? box.maxHeight : w;
      return Container(
        color: Colors.white.withValues(alpha: 0.05),
        child: Stack(
          fit: StackFit.expand,
          children: [
            if (doc.thumb)
              FutureBuilder<String?>(
                key: ValueKey('t${doc.id}'),
                future: TgMedia.instance.file(doc, thumb: true),
                builder: (context, snap) {
                  final p = snap.data;
                  if (p == null) return const SizedBox();
                  return Image.file(File(p),
                      fit: BoxFit.cover,
                      cacheWidth: (w * dpr).round().clamp(32, 480),
                      gaplessPlayback: true);
                },
              ),
            // Panelda — faqat kichik rasm (to'liq GIF yuklanmaydi ham);
            // katta ko'rinishda ([loop]) — harakatlanadi.
            if (play && (loop || !doc.thumb))
              _Deferred(
                key: ValueKey('g${doc.id}'),
                builder: (context) => FutureBuilder<String?>(
                  future: TgMedia.instance.file(doc),
                  builder: (context, snap) {
                    final p = snap.data;
                    if (p == null) return const SizedBox();
                    return TgAnimView(
                      key: ValueKey(p),
                      path: p,
                      size: w,
                      height: h,
                      gif: true,
                      panel: !loop,
                      live: loop,
                      fallback: const SizedBox(),
                    );
                  },
                ),
              ),
          ],
        ),
      );
    });
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

/// Nomdagi GIF kaliti (`..._g<id>_<ah>_<dc>_<fr>.mp4`).
final _gifTag = RegExp(r'_g([0-9a-f]{1,16})_([0-9a-f]{1,16})_(\d{1,3})_([0-9a-f]{2,120})\.mp4$');

/// Telegram ilovasidagidek: GIF o'z Telegram hisobi bilan to'g'ridan-
/// to'g'ri Telegram serveridan (bot chati, worker ishtirokisiz).
Future<File?> _directGif(String name) async {
  final m = _gifTag.firstMatch(name);
  if (m == null) return null;
  // Diskda bo'lsa — navbatsiz, darhol.
  final dir = TelegramService.instance.mediaDir;
  if (dir.isNotEmpty) {
    final id = BigInt.parse(m.group(1)!, radix: 16).toSigned(64);
    final f = File('$dir/$id');
    if (f.existsSync()) return f;
  }
  if (!TelegramService.instance.authorized) return null;
  try {
    final j = await NativePool.files.call('rust_tg_gif_direct',
        arg: jsonEncode({
          'id': m.group(1),
          'ah': m.group(2),
          'dc': m.group(3),
          'fr': m.group(4),
        }));
    final p = j['path'];
    if (p is String && File(p).existsSync()) return File(p);
  } catch (_) {}
  return null;
}

/// Fayl haqiqiy MP4mi (`....ftyp`) — yarim/buzuq yuklangan fayl
/// ("GIF qorayib yotibdi") keshda qolmasin.
bool _looksLikeMp4(File f) {
  try {
    final r = f.openSync();
    try {
      final head = r.readSync(12);
      return head.length >= 8 &&
          head[4] == 0x66 && // f
          head[5] == 0x74 && // t
          head[6] == 0x79 && // y
          head[7] == 0x70; // p
    } finally {
      r.closeSync();
    }
  } catch (_) {
    return false;
  }
}

/// GIF faqat Telegram serveridan, ko'ruvchining o'z hisobi bilan
/// (`_directGif`). TALAB (foydalanuvchi): "maxfiy kanal va B2'siz" —
/// bot chati orqali olish (`fetchBytes`) olib tashlandi: u bot chatini
/// band qilib, tozalashda videolar nusxasini ham o'chirib yuborardi.
Future<File?> _fetchChatFile(String name) async {
  final f = await _directGif(name);
  if (f != null && _looksLikeMp4(f)) return f;
  if (f != null) {
    try {
      f.deleteSync();
    } catch (_) {}
  }
  _chatFiles.remove(name);
  return null;
}


class _TgGifMessageState extends State<TgGifMessage> {
  String? _path;
  double _ratio = 1.4;
  bool _failed = false;
  int _tries = 0;

  @override
  void initState() {
    super.initState();
    _open();
  }

  @override
  void didUpdateWidget(TgGifMessage old) {
    super.didUpdateWidget(old);
    // Ro'yxatdagi katak boshqa xabarga qayta ishlatildi.
    if (old.fileName != widget.fileName) {
      _path = null;
      _failed = false;
      _tries = 0;
      _open();
    }
  }

  Future<void> _retry(String name) async {
    // Yangi yuborilgan GIF hali tayyor bo'lmasligi yoki tarmoq uzilgan
    // bo'lishi mumkin — qayta uriniladi.
    if (_tries++ < 10) {
      await Future<void>.delayed(
          Duration(seconds: (2 + _tries * 3).clamp(2, 30)));
      if (mounted && name == widget.fileName) return _open();
      return;
    }
    if (mounted) setState(() => _failed = true);
  }

  Future<void> _open() async {
    final name = widget.fileName;
    File? f;
    try {
      f = await tgChatFile(name).timeout(const Duration(seconds: 90));
    } catch (_) {
      _chatFiles.remove(name);
    }
    if (!mounted || name != widget.fileName) return;
    if (f == null) return _retry(name);
    // Nisbat — fayl sarlavhasidan (ochmasdan), katak shunga qarab.
    final j = await NativePool.render
        .call('rust_anim_probe', arg: f.path);
    if (!mounted || name != widget.fileName) return;
    final w = (j['w'] as num?)?.toDouble() ?? 0;
    final h = (j['h'] as num?)?.toDouble() ?? 0;
    if (w <= 0 || h <= 0) {
      // MP4 emas yoki buzuq — o'chiriladi va qayta yuklanadi.
      _chatFiles.remove(name);
      try {
        await f.delete();
      } catch (_) {}
      return _retry(name);
    }
    setState(() {
      _ratio = (w / h).clamp(0.4, 3.0);
      _path = f!.path;
    });
  }

  @override
  Widget build(BuildContext context) {
    final p = _path;
    final w = widget.maxWidth;
    final h = w / _ratio;
    return ClipRRect(
      borderRadius: BorderRadius.circular(14),
      child: SizedBox(
        width: w,
        height: h,
        child: p == null
            ? Container(
                color: Colors.white.withValues(alpha: 0.06),
                alignment: Alignment.center,
                child: _failed
                    // Bosilsa qaytadan yuklanadi.
                    ? GestureDetector(
                        behavior: HitTestBehavior.opaque,
                        onTap: () {
                          setState(() {
                            _failed = false;
                            _tries = 0;
                          });
                          _open();
                        },
                        child: Icon(Icons.refresh_rounded,
                            color: Colors.white.withValues(alpha: 0.45),
                            size: 34),
                      )
                    : const SizedBox(
                        width: 22,
                        height: 22,
                        child: CircularProgressIndicator(
                            strokeWidth: 2, color: Colors.white54),
                      ),
              )
            // Turadi (birinchi kadr); bosilsa — tepada katta bo'lib
            // harakatlanadi (ilova ichidagi dekoder bilan).
            : GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: () =>
                    TgHoldPreview.showTap(context, path: p, ratio: _ratio),
                child: TgAnimView(
                  key: ValueKey(p),
                  path: p,
                  size: w,
                  height: h,
                  gif: true,
                  fallback:
                      Container(color: Colors.white.withValues(alpha: 0.06)),
                ),
              ),
      ),
    );
  }
}

// ═══════════════════════════════════════════════════════════════
//  OLDINDAN TAYYORLASH
// ═══════════════════════════════════════════════════════════════

final Set<String> _prefetched = {};

/// Chat yoki izohlardagi stiker, GIF va maxsus emojilarni OLDINDAN
/// tayyorlaydi (hujjat topiladi, fayl diskka olinadi) — ekranga
/// chiqqanda yuklanib o'tirmaydi. Eng yangilari (ro'yxat oxiri) birinchi;
/// ko'pi 40 xabar, GIF'lardan 12 tasi.
void tgPrefetch(Iterable<({String type, String file, String body})> items) {
  final list = items.toList().reversed.take(40);
  var gifs = 0;
  for (final m in list) {
    if (m.type == 'sticker' && m.file.isNotEmpty) {
      if (_prefetched.add('s:${m.file}')) {
        unawaited(TgMedia.instance.stickerByRef(m.file).then((d) {
          if (d != null) return TgMedia.instance.file(d);
          return null;
        }).catchError((_) => null));
      }
    } else if (m.type == 'gif' && m.file.isNotEmpty && gifs < 12) {
      gifs++;
      if (_prefetched.add('g:${m.file}')) {
        unawaited(tgChatFile(m.file).catchError((_) => null));
      }
    }
    if (m.body.contains('[ce:')) {
      for (final t in customEmojiToken.allMatches(m.body)) {
        final id = t.group(1)!;
        if (!_prefetched.add('e:$id')) continue;
        unawaited(TgMedia.instance.customEmoji(id).then((d) {
          if (d != null) return TgMedia.instance.file(d);
          return null;
        }).catchError((_) => null));
      }
    }
  }
}

// ═══════════════════════════════════════════════════════════════
//  GURUH: BIR NECHTA ANIMATSIYA — BITTA YUZA (Telegram kabi)
// ═══════════════════════════════════════════════════════════════
//
// Telegram (`DrawingInBackgroundThreadDrawable`) ekrandagi emojilarni
// bittalab chizmaydi: bir qatordagi hammasi fon oqimida BITTA rasmga
// chiziladi va ekranga bitta rasm chiqadi. Bu yerda ham: [TgAnimBatch]
// ichidagi har bir [TgAnimView] o'z yuzasini ochmaydi — o'rnini
// (to'rtburchagini) guruhga aytadi, guruh esa hammasini Rust'da bitta
// yuzaga chizdiradi (`rust_player_open_multi`). 8 ta yuza o'rniga bitta
// — grafik protsessor yuki ancha kam, kuchsiz telefonda ham silliq.

class TgAnimBatch extends StatefulWidget {
  final Widget child;
  const TgAnimBatch({super.key, required this.child});

  static _TgAnimBatchState? _of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<_BatchScope>()?.state;

  @override
  State<TgAnimBatch> createState() => _TgAnimBatchState();
}

class _BatchScope extends InheritedWidget {
  final _TgAnimBatchState state;
  const _BatchScope({required this.state, required super.child});

  @override
  bool updateShouldNotify(_BatchScope old) => !identical(old.state, state);
}

class _BatchItem {
  final String path;
  final Rect rect;
  final bool once;
  _BatchItem(this.path, this.rect, this.once);
}

class _TgAnimBatchState extends State<TgAnimBatch> {
  static const _ch = MethodChannel('aru/anim');
  final Map<_TgAnimViewState, _BatchItem> _items = {};
  Timer? _debounce;
  bool _dead = false;
  int _player = 0;
  int? _texture;
  int _gen = 0;
  ValueListenable<TickerModeData>? _tm;
  bool _enabled = true;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final tm = TickerMode.getValuesNotifier(context);
    if (!identical(tm, _tm)) {
      _tm?.removeListener(_onTm);
      _tm = tm..addListener(_onTm);
      _enabled = tm.value.enabled;
    }
  }

  void _onTm() {
    _enabled = _tm?.value.enabled ?? true;
    if (_player != 0) AnimPlayers.setPlaying(_player, _enabled);
  }

  void put(_TgAnimViewState who, String path, Rect rect) {
    final old = _items[who];
    if (old != null && old.path == path && old.rect == rect) return;
    _items[who] = _BatchItem(path, rect, who.widget.once);
    _schedule();
  }

  void remove(_TgAnimViewState who) {
    if (_items.remove(who) != null && !_dead) _schedule();
  }

  void _schedule([int ms = 60]) {
    _debounce?.cancel();
    _debounce = Timer(Duration(milliseconds: ms), _rebuild);
  }

  /// Yuza joyi (guruh ichida) — faqat animatsiyalar egallagan qism.
  Rect _texRect = Rect.zero;
  Rect _nextRect = Rect.zero;

  Future<void> _rebuild() async {
    if (_dead || !mounted) return;
    // SURISHDA QOTISH (foydalanuvchi: "izoh va support chatni surganda
    // qotyapti"): ro'yxat tez surilayotganda har yangi xabar uchun yuza
    // ochilardi (platforma oqimida `SurfaceProducer`, Rust'da kadrlar).
    // Flutter tavsiyasi bo'yicha — surish sekinlashguncha kutiladi.
    if (Scrollable.recommendDeferredLoadingForContext(context)) {
      _schedule(150);
      return;
    }
    final gen = ++_gen;
    final box = context.findRenderObject() as RenderBox?;
    if (box == null || !box.hasSize || _items.isEmpty) {
      _swap(0, null);
      return;
    }
    final dpr = MediaQuery.devicePixelRatioOf(context);
    // Ekran zichligida (3x gacha) — kichikroq chizilib cho'zilsa
    // emoji/stiker xira ko'rinardi.
    final scale = math.min(dpr, DevicePerf.low ? 2.0 : 3.0);
    // Yuza butun pufakcha emas — faqat animatsiyalarni o'rab turgan
    // to'rtburchak. Ilgari bitta emojili matn xabari uchun ham butun
    // pufakcha kattaligida (masalan 900x600 px) yuza ochilib, har kadrda
    // to'liq ko'chirilardi — grafik protsessor yuki surishni qotirardi.
    var u = _items.values.first.rect;
    for (final it in _items.values) {
      u = u.expandToInclude(it.rect);
    }
    u = u.intersect(Offset.zero & box.size);
    final w = (u.width * scale).round();
    final h = (u.height * scale).round();
    if (w <= 0 || h <= 0) return;
    final items = [
      for (final it in _items.values)
        {
          'path': it.path,
          'x': ((it.rect.left - u.left) * scale).round(),
          'y': ((it.rect.top - u.top) * scale).round(),
          'w': (it.rect.width * scale).round(),
          'h': (it.rect.height * scale).round(),
          'once': it.once,
        }
    ];
    _nextRect = u;
    final hasOnce = _items.values.any((it) => it.once);
    final j = await NativePool.render.call('rust_player_open_multi',
        arg: jsonEncode({'w': w, 'h': h, 'items': items}));
    final id = (j['id'] as num?)?.toInt() ?? 0;
    if (id <= 0) return;
    if (_dead || gen != _gen) {
      AnimPlayers.free(id);
      return;
    }
    int? tex;
    try {
      tex = await _ch.invokeMethod<int>('create', {'player': id, 'w': w, 'h': h});
    } catch (_) {
      tex = null;
    }
    if (tex == null) {
      AnimPlayers.free(id);
      _failAll();
      return;
    }
    if (_dead || gen != _gen) {
      unawaited(_ch
          .invokeMethod('dispose', {'texture': tex})
          .catchError((_) => null)
          .whenComplete(() => AnimPlayers.free(id)));
      return;
    }
    AnimPlayers.setPlaying(id, _enabled);
    // Yangi rasm birinchi kadri chiqqach almashtiriladi (miltillamasin).
    var waited = 0;
    Timer.periodic(const Duration(milliseconds: 16), (t) {
      waited += 16;
      if (_dead || gen != _gen) {
        t.cancel();
        if (_texture != tex) {
          unawaited(_ch
              .invokeMethod('dispose', {'texture': tex})
              .catchError((_) => null)
              .whenComplete(() => AnimPlayers.free(id)));
        }
        return;
      }
      if (AnimPlayers.drawn(id) || waited > 3000) {
        t.cancel();
        _texRect = _nextRect;
        _swap(id, tex);
        // Bir martalik stiker/GIF — ekranda to'liq ko'ringanda.
        if (hasOnce) {
          (_watch ??= _OnceWatch(context, replay)).attach(fresh: true);
        }
      }
    });
  }

  /// Texture yo'li umuman ishlamadi — hamma animatsiya o'zi chizadi.
  void _failAll() {
    _TgAnimViewState._texBroken = true;
    final list = _items.keys.toList();
    _items.clear();
    for (final v in list) {
      v._batchFailed();
    }
  }

  _OnceWatch? _watch;

  void replay() {
    if (_player != 0) AnimPlayers.replay(_player);
  }

  void _swap(int id, int? tex) {
    final oldId = _player;
    final oldTex = _texture;
    _player = id;
    _texture = tex;
    if (mounted && !_dead) setState(() {});
    if (oldId != 0) {
      AnimPlayers.setPlaying(oldId, false);
      if (oldTex == null) {
        AnimPlayers.free(oldId);
      } else {
        unawaited(_ch
            .invokeMethod('dispose', {'texture': oldTex})
            .catchError((_) => null)
            .whenComplete(() => AnimPlayers.free(oldId)));
      }
    }
  }

  @override
  void dispose() {
    _dead = true;
    _watch?.detach();
    _debounce?.cancel();
    _tm?.removeListener(_onTm);
    _swap(0, null);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final tex = _texture;
    return _BatchScope(
      state: this,
      child: Stack(
        children: [
          widget.child,
          if (tex != null)
            Positioned.fromRect(
              rect: _texRect,
              child: IgnorePointer(
                child: Texture(
                    textureId: tex, filterQuality: FilterQuality.medium),
              ),
            ),
        ],
      ),
    );
  }
}

/// "Ekranda TO'LIQ ko'rindi" kuzatuvchisi (bir martalik animatsiyalar).
///
/// TALAB (foydalanuvchi): "yuborilgan GIF va stikerlar ekranda
/// ko'ringanda bir marta animatsiya bo'lsin; ekranni surganda yo'qolib,
/// qayta chiqsa yana bir marta".
///
/// Ro'yxat surilganda joy tekshiriladi: to'liq ko'ringan paytda
/// [onShow] bir marta chaqiriladi; element ko'rinish maydonidan butunlay
/// chiqqach yana "o'qlanadi".
class _OnceWatch {
  final BuildContext context;
  final VoidCallback onShow;
  _OnceWatch(this.context, this.onShow);

  ScrollableState? _scroll;
  ScrollPosition? _pos;
  bool _armed = true;

  /// [fresh] — yangi o'yinchi: ko'rinib turgan bo'lsa darhol o'ynaydi.
  void attach({bool fresh = false}) {
    if (!context.mounted) return;
    if (fresh) _armed = true;
    if (_pos == null) {
      _scroll = Scrollable.maybeOf(context);
      _pos = _scroll?.position;
      _pos?.addListener(check);
    }
    WidgetsBinding.instance.addPostFrameCallback((_) => check());
  }

  void detach() {
    _pos?.removeListener(check);
    _pos = null;
    _scroll = null;
  }

  void check() {
    if (!context.mounted) return;
    final me = context.findRenderObject() as RenderBox?;
    if (me == null || !me.attached || !me.hasSize) return;
    final vb = _scroll?.context.findRenderObject() as RenderBox?;
    final view = vb != null && vb.attached && vb.hasSize
        ? vb.localToGlobal(Offset.zero) & vb.size
        : Offset.zero & MediaQuery.sizeOf(context);
    final r = me.localToGlobal(Offset.zero) & me.size;
    final seen = r.intersect(view);
    final full = seen.width > 0 &&
        seen.height >= math.min(r.height, view.height) - 1;
    if (full) {
      if (_armed) {
        _armed = false;
        onShow();
      }
    } else if (!r.overlaps(view)) {
      _armed = true;
    }
  }
}

/// Bosilganini sezadi, lekin ota-onaning bosishini (tanlash, menyu)
/// "o'g'irlamaydi" — `Listener` bosish musobaqasida qatnashmaydi.
class _TapReplay extends StatefulWidget {
  final VoidCallback onTap;
  final Widget child;
  const _TapReplay({required this.onTap, required this.child});

  @override
  State<_TapReplay> createState() => _TapReplayState();
}

class _TapReplayState extends State<_TapReplay> {
  Offset? _down;
  DateTime _at = DateTime.now();

  @override
  Widget build(BuildContext context) {
    return Listener(
      behavior: HitTestBehavior.opaque,
      onPointerDown: (e) {
        _down = e.position;
        _at = DateTime.now();
      },
      onPointerUp: (e) {
        final d = _down;
        _down = null;
        if (d != null &&
            (e.position - d).distance < 12 &&
            DateTime.now().difference(_at).inMilliseconds < 400) {
          widget.onTap();
        }
      },
      onPointerCancel: (_) => _down = null,
      child: widget.child,
    );
  }
}
