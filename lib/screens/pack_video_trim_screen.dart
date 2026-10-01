// lib/screens/pack_video_trim_screen.dart — VIDEONI KESISH (tahrirlash).
//
// Telegram'dagidek: video aylanib turadi, foydalanuvchi qaysi bo'lagi
// kerakligini tanlaydi. Bo'lakning uzunligi tur bo'yicha cheklangan
// (`kPackMaxSeconds`). Kesishni telefonda emas, Actions bajaradi
// (`tool/packs/arunorm.py`): telefonga og'ir ish tushmaydi.
//
// Eslatma: 5 MB chegarasi YUKLANADIGAN faylga tegishli (kesish uni
// kichraytirmaydi).

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:open_filex/open_filex.dart';
import 'package:video_player/video_player.dart';

import '../services/pack_service.dart';
import '../widgets/glass.dart';

/// Tur bo'yicha eng uzun bo'lak (soniya); 0 — uzunlik cheklanmaydi (GIF:
/// faqat hajm 5 MB dan oshmasin). Serverda (`tool/packs/arunorm.py`) uzunlik
/// tekshirilmaydi, faqat 5 MB.
int packMaxSeconds(String kind) => switch (kind) {
      PackKind.emoji => 8,
      PackKind.gif => 0,
      _ => 12,
    };

String fmtMs(int ms) {
  final s = ms / 1000;
  final m = s ~/ 60;
  final r = (s - m * 60).toStringAsFixed(1).padLeft(4, '0');
  return '$m:$r';
}

/// Videodan tekis oraliqda kadrlar (JPEG) oladi — vaqt chizig'i uchun.
/// Android'da `aru/thumb` kanalining `frames` usuli (MediaMetadataRetriever).
/// Kanal yo'q bo'lsa (test, boshqa platforma) — bo'sh ro'yxat.
Future<List<Uint8List?>> grabVideoFrames(String path,
    {int count = 8, int maxWidth = 160}) async {
  try {
    final r = await const MethodChannel('aru/thumb').invokeMethod<List<dynamic>>(
        'frames', {
      'path': path,
      'count': count,
      'maxWidth': maxWidth,
      'quality': 60,
    });
    if (r == null) return const [];
    return [for (final e in r) e is Uint8List ? e : null];
  } catch (_) {
    return const [];
  }
}

/// CapCut uslubidagi kesish oynasi: tepada aylanib turuvchi ko'rinish,
/// pastda kadrlardan iborat vaqt chizig'i. Chetlarini yoki oynaning o'zini
/// sudrab bo'lak tanlanadi. Eng uzun bo'lak tur bo'yicha cheklangan.
class PackVideoTrimScreen extends StatefulWidget {
  final String path;
  final String kind;
  final int start;
  final int end;

  /// Ovoz saqlansinmi (faqat GIF to'plamida ma'noli).
  final bool sound;
  const PackVideoTrimScreen({
    super.key,
    required this.path,
    required this.kind,
    this.start = 0,
    this.end = 0,
    this.sound = true,
  });

  @override
  State<PackVideoTrimScreen> createState() => _PackVideoTrimScreenState();
}

enum _Drag { none, left, right, body }

class _PackVideoTrimScreenState extends State<PackVideoTrimScreen> {
  static const double _minMs = 500;
  static const double _handleW = 20;
  static const int _frameCount = 8;

  VideoPlayerController? _c;
  double _dur = 0;
  double _a = 0;
  double _b = 0;
  double _pos = 0;
  String? _error;
  List<Uint8List?> _frames = const [];
  _Drag _drag = _Drag.none;
  late bool _sound = widget.sound;

  bool get _canSound => widget.kind == PackKind.gif;

  int get _maxMs => packMaxSeconds(widget.kind) * 1000;
  double get _maxSel =>
      _maxMs <= 0 || _dur < _maxMs ? _dur : _maxMs.toDouble();
  double get _minSel => _dur < _minMs ? _dur : _minMs;

  @override
  void initState() {
    super.initState();
    unawaited(_init());
  }

  Future<void> _init() async {
    final c = VideoPlayerController.file(File(widget.path));
    try {
      await c.initialize();
    } catch (_) {
      await c.dispose();
      if (mounted) setState(() => _error = 'Videoni ochib bo\'lmadi');
      return;
    }
    if (!mounted) {
      await c.dispose();
      return;
    }
    c.setVolume(_canSound && _sound ? 1 : 0);
    _dur = c.value.duration.inMilliseconds.toDouble();
    if (_dur <= 0) {
      await c.dispose();
      setState(() => _error = 'Video davomiyligi noma\'lum');
      return;
    }
    _a = widget.end > widget.start ? widget.start.toDouble() : 0;
    _b = widget.end > widget.start ? widget.end.toDouble() : _maxSel;
    _a = _a.clamp(0.0, _dur).toDouble();
    _b = _b.clamp(_a, _dur).toDouble();
    if (_b - _a > _maxSel) _b = _a + _maxSel;
    _c = c;
    c.addListener(_tick);
    unawaited(c.seekTo(Duration(milliseconds: _a.round())));
    unawaited(c.play());
    setState(() {});
    final f = await grabVideoFrames(widget.path, count: _frameCount);
    if (mounted) setState(() => _frames = f);
  }

  /// Bo'lak oxiriga yetsa — boshiga qaytadi (aylanib turadi).
  void _tick() {
    final c = _c;
    if (c == null || !c.value.isInitialized || !mounted) return;
    final p = c.value.position.inMilliseconds.toDouble();
    if (_drag == _Drag.none && c.value.isPlaying && p >= _b - 30) {
      unawaited(c.seekTo(Duration(milliseconds: _a.round())));
    }
    if ((p - _pos).abs() > 16) setState(() => _pos = p);
  }

  @override
  void dispose() {
    _c?.removeListener(_tick);
    _c?.dispose();
    super.dispose();
  }

  void _seek(double ms) {
    final c = _c;
    if (c == null) return;
    _pos = ms;
    unawaited(c.seekTo(Duration(milliseconds: ms.round())));
  }

  _Drag _hit(double x, double w) {
    final ax = _a / _dur * w;
    final bx = _b / _dur * w;
    if ((x - ax).abs() <= 36 && (x - ax).abs() <= (x - bx).abs()) {
      return _Drag.left;
    }
    if ((x - bx).abs() <= 36) return _Drag.right;
    if (x > ax && x < bx) return _Drag.body;
    return _Drag.none;
  }

  void _onStart(DragStartDetails d, double w) {
    _drag = _hit(d.localPosition.dx, w);
    if (_drag != _Drag.none) unawaited(_c?.pause());
  }

  void _onUpdate(DragUpdateDetails d, double w) {
    if (_drag == _Drag.none) return;
    final dm = d.delta.dx / w * _dur;
    var a = _a, b = _b;
    switch (_drag) {
      case _Drag.left:
        a = (a + dm).clamp(0.0, b - _minSel).toDouble();
        if (b - a > _maxSel) a = b - _maxSel;
      case _Drag.right:
        b = (b + dm).clamp(a + _minSel, _dur).toDouble();
        if (b - a > _maxSel) b = a + _maxSel;
      case _Drag.body:
        final len = b - a;
        a = (a + dm).clamp(0.0, _dur - len).toDouble();
        b = a + len;
      case _Drag.none:
        break;
    }
    setState(() {
      _a = a;
      _b = b;
    });
    _seek(_drag == _Drag.right ? (b - 400 < a ? a : b - 400) : a);
  }

  void _onEnd() {
    if (_drag == _Drag.none) return;
    _drag = _Drag.none;
    _seek(_a);
    unawaited(_c?.play());
  }

  void _nudge({required bool start, required double dm}) {
    var a = _a, b = _b;
    if (start) {
      a = (a + dm).clamp(0.0, b - _minSel).toDouble();
      if (b - a > _maxSel) a = b - _maxSel;
    } else {
      b = (b + dm).clamp(a + _minSel, _dur).toDouble();
      if (b - a > _maxSel) b = a + _maxSel;
    }
    setState(() {
      _a = a;
      _b = b;
    });
    final c = _c;
    if (c == null) return;
    unawaited(c.pause());
    _seek(start ? a : (b - 400 < a ? a : b - 400));
  }

  /// Bo'lakni imkon qadar uzun qiladi (chegaragacha), boshini saqlab.
  void _fill() {
    var a = _a;
    var b = a + _maxSel;
    if (b > _dur) {
      b = _dur;
      a = (b - _maxSel).clamp(0.0, b).toDouble();
    }
    setState(() {
      _a = a;
      _b = b;
    });
    _seek(a);
    unawaited(_c?.play());
  }

  void _togglePlay() {
    final c = _c;
    if (c == null) return;
    if (c.value.isPlaying) {
      unawaited(c.pause());
    } else {
      if (c.value.position.inMilliseconds >= _b - 30 ||
          c.value.position.inMilliseconds < _a) {
        _seek(_a);
      }
      unawaited(c.play());
    }
    setState(() {});
  }

  Widget _step(String label, bool start) {
    Widget b(IconData i, double dm) => InkResponse(
          onTap: () => _nudge(start: start, dm: dm),
          radius: 22,
          child: Padding(
            padding: const EdgeInsets.all(8),
            child: Icon(i, color: Colors.white, size: 22),
          ),
        );
    return Container(
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(12),
      ),
      padding: const EdgeInsets.symmetric(horizontal: 4),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          b(Icons.chevron_left_rounded, -100),
          Text(label,
              style: const TextStyle(color: Colors.white70, fontSize: 12.5)),
          b(Icons.chevron_right_rounded, 100),
        ],
      ),
    );
  }

  /// Telegram uslubidagi kadrlar tasmasi: och kulrang ramka, ikki chetda
  /// tutqichlar, tanlanmagan joy xira, ingichka o'ynash chizig'i.
  Widget _timeline(double w) {
    const h = 56.0;
    final ax = _a / _dur * w;
    final bx = _b / _dur * w;
    final px = (_pos.clamp(_a, _b) / _dur * w).clamp(ax, bx).toDouble();
    const light = Color(0xFFBDBDBD);
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTapUp: (d) {
        final ms = (d.localPosition.dx / w * _dur).clamp(_a, _b).toDouble();
        _seek(ms);
      },
      onHorizontalDragStart: (d) => _onStart(d, w),
      onHorizontalDragUpdate: (d) => _onUpdate(d, w),
      onHorizontalDragEnd: (_) => _onEnd(),
      onHorizontalDragCancel: _onEnd,
      child: SizedBox(
        height: h + 20,
        width: w,
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            Positioned(
              left: 0,
              right: 0,
              top: 10,
              height: h,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(8),
                child: Row(
                  children: [
                    for (var i = 0; i < _frameCount; i++)
                      Expanded(
                        child: i < _frames.length && _frames[i] != null
                            ? Image.memory(_frames[i]!,
                                fit: BoxFit.cover,
                                height: h,
                                gaplessPlayback: true)
                            : Container(
                                height: h, color: const Color(0xFF151515)),
                      ),
                  ],
                ),
              ),
            ),
            Positioned(
                left: 0,
                width: ax,
                top: 10,
                height: h,
                child: Container(color: Colors.black.withValues(alpha: 0.7))),
            Positioned(
                left: bx,
                right: 0,
                top: 10,
                height: h,
                child: Container(color: Colors.black.withValues(alpha: 0.7))),
            // tanlangan bo'lak ramkasi
            Positioned(
              left: ax,
              width: bx - ax,
              top: 10,
              height: h,
              child: const DecoratedBox(
                decoration: BoxDecoration(
                  border: Border.symmetric(
                      horizontal: BorderSide(color: light, width: 3)),
                ),
              ),
            ),
            // tutqichlar
            for (final e in [(ax - _handleW, true), (bx, false)])
              Positioned(
                left: e.$1,
                width: _handleW,
                top: 10,
                height: h,
                child: Container(
                  decoration: BoxDecoration(
                    color: light,
                    borderRadius: e.$2
                        ? const BorderRadius.horizontal(
                            left: Radius.circular(8))
                        : const BorderRadius.horizontal(
                            right: Radius.circular(8)),
                  ),
                  child: const Center(
                    child: SizedBox(
                      width: 3,
                      height: 22,
                      child: DecoratedBox(
                          decoration: BoxDecoration(
                              color: Color(0xFF3A3A3A),
                              borderRadius:
                                  BorderRadius.all(Radius.circular(2)))),
                    ),
                  ),
                ),
              ),
            // o'ynash chizig'i
            Positioned(
              left: px - 1.5,
              width: 3,
              top: 4,
              height: h + 12,
              child: const DecoratedBox(
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.all(Radius.circular(2)),
                  boxShadow: [BoxShadow(color: Colors.black54, blurRadius: 3)],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _pill(Widget child, {VoidCallback? onTap}) => GestureDetector(
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 9),
          decoration: BoxDecoration(
            color: const Color(0xFF1C1C1E),
            borderRadius: BorderRadius.circular(18),
          ),
          child: child,
        ),
      );

  @override
  Widget build(BuildContext context) {
    final c = _c;
    final len = (_b - _a).round();
    final maxS = packMaxSeconds(widget.kind);
    final atMax = _b - _a >= _maxSel - 1;
    return Scaffold(
      backgroundColor: Colors.black,
      appBar: AppBar(
        backgroundColor: Colors.black,
        elevation: 0,
        iconTheme: const IconThemeData(color: Colors.white),
        title: Text(
            widget.kind == PackKind.gif
                ? 'GIF'
                : widget.kind == PackKind.emoji
                    ? 'Emoji'
                    : 'Stiker',
            style: const TextStyle(color: Colors.white, fontSize: 19)),
      ),
      body: _error != null
          ? Center(
              child: Text(_error!,
                  style: const TextStyle(color: AppColors.danger)))
          : c == null
              ? const Center(
                  child: CircularProgressIndicator(
                      strokeWidth: 2.4, color: Colors.white54))
              : SafeArea(
                  child: Column(
                    children: [
                      Expanded(
                        child: GestureDetector(
                          behavior: HitTestBehavior.opaque,
                          onTap: _togglePlay,
                          child: Center(
                            child: Stack(
                              alignment: Alignment.center,
                              children: [
                                AspectRatio(
                                  aspectRatio: c.value.aspectRatio,
                                  child: VideoPlayer(c),
                                ),
                                if (!c.value.isPlaying)
                                  Container(
                                    width: 64,
                                    height: 64,
                                    decoration: const BoxDecoration(
                                        color: Colors.black54,
                                        shape: BoxShape.circle),
                                    child: const Icon(
                                        Icons.play_arrow_rounded,
                                        color: Colors.white,
                                        size: 44),
                                  ),
                              ],
                            ),
                          ),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.fromLTRB(16, 0, 16, 0),
                        child: Row(
                          children: [
                            GestureDetector(
                              onTap: !_canSound
                                  ? () => ScaffoldMessenger.of(context)
                                      .showSnackBar(const SnackBar(
                                          content: Text(
                                              'Ovoz faqat GIF to\'plamida saqlanadi')))
                                  : () {
                                      setState(() => _sound = !_sound);
                                      unawaited(_c?.setVolume(_sound ? 1 : 0));
                                    },
                              child: Container(
                                width: 44,
                                height: 44,
                                decoration: BoxDecoration(
                                    color: _canSound && _sound
                                        ? AppColors.accent
                                        : const Color(0xFF1C1C1E),
                                    shape: BoxShape.circle),
                                child: Icon(
                                    _canSound && _sound
                                        ? Icons.volume_up_rounded
                                        : Icons.volume_off_rounded,
                                    color: _canSound
                                        ? Colors.white
                                        : Colors.white54,
                                    size: 22),
                              ),
                            ),
                            const Spacer(),
                            _pill(
                              Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Text('${(len / 1000).toStringAsFixed(1)} s',
                                      style: TextStyle(
                                          color: atMax
                                              ? AppColors.accent
                                              : Colors.white,
                                          fontSize: 16,
                                          fontWeight: FontWeight.w700)),
                                  if (maxS > 0)
                                    Text('  ·  eng ko\'pi $maxS s',
                                        style: const TextStyle(
                                            color: Colors.white54,
                                            fontSize: 14)),
                                ],
                              ),
                              onTap: atMax ? null : _fill,
                            ),
                            const Spacer(),
                            const SizedBox(width: 44),
                          ],
                        ),
                      ),
                      const SizedBox(height: 14),
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 26),
                        child: LayoutBuilder(
                          builder: (_, cs) => _timeline(cs.maxWidth),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.fromLTRB(26, 0, 26, 0),
                        child: Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            Text(fmtMs(_a.round()),
                                style: const TextStyle(
                                    color: Colors.white54, fontSize: 12)),
                            Text(fmtMs(_b.round()),
                                style: const TextStyle(
                                    color: Colors.white54, fontSize: 12)),
                          ],
                        ),
                      ),
                      const SizedBox(height: 10),
                      Padding(
                        padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
                        child: Row(
                          children: [
                            _step('Boshi', true),
                            const SizedBox(width: 8),
                            _step('Oxiri', false),
                            const Spacer(),
                            GestureDetector(
                              onTap: () => Navigator.of(context).pop(
                                  (_a.round(), _b.round(), _canSound && _sound)),
                              child: Container(
                                width: 56,
                                height: 56,
                                decoration: const BoxDecoration(
                                  color: AppColors.accent,
                                  shape: BoxShape.circle,
                                  boxShadow: [
                                    BoxShadow(
                                        color: Colors.black54, blurRadius: 10)
                                  ],
                                ),
                                child: const Icon(Icons.check_rounded,
                                    color: Colors.white, size: 30),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
    );
  }
}

/// Kartadagi video ko'rinishi: birinchi kadr (to'xtab turadi), bosilsa o'ynaydi.
class PackVideoPreview extends StatefulWidget {
  final String path;
  final double size;

  /// Ovoz bilan ko'rsatish (admin tekshiruvi): karnay tugmasi chiqadi.
  final bool sound;
  const PackVideoPreview(
      {super.key, required this.path, this.size = 96, this.sound = false});

  @override
  State<PackVideoPreview> createState() => _PackVideoPreviewState();
}

class _PackVideoPreviewState extends State<PackVideoPreview> {
  VideoPlayerController? _c;
  String? _err;
  late bool _on = widget.sound;

  @override
  void initState() {
    super.initState();
    unawaited(_init());
  }

  Future<void> _init() async {
    final c = VideoPlayerController.file(File(widget.path));
    try {
      await c.initialize();
      await c.setVolume(widget.sound ? 1 : 0);
      await c.setLooping(true);
    } catch (e) {
      await c.dispose();
      if (mounted) {
        final t = '$e'.replaceAll(RegExp(r'\s+'), ' ');
        setState(() => _err = t.length > 90 ? t.substring(0, 90) : t);
      }
      return;
    }
    if (!mounted) {
      await c.dispose();
      return;
    }
    setState(() => _c = c);
  }

  @override
  void dispose() {
    _c?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = _c;
    return GestureDetector(
      onTap: c == null
          ? null
          : () => setState(() => c.value.isPlaying ? c.pause() : c.play()),
      child: SizedBox(
        width: widget.size,
        height: widget.size,
        child: c == null
            ? Center(
                child: _err == null
                    ? const Icon(Icons.movie_outlined, color: Colors.white38)
                    : Padding(
                        padding: const EdgeInsets.all(10),
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            const Icon(Icons.error_outline_rounded,
                                color: AppColors.danger, size: 26),
                            const SizedBox(height: 6),
                            Text('Video ochilmadi: $_err',
                                textAlign: TextAlign.center,
                                maxLines: 4,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                    color: Colors.white54, fontSize: 11)),
                            TextButton(
                              onPressed: () =>
                                  OpenFilex.open(widget.path),
                              child: const Text('Tashqi ilovada ochish'),
                            ),
                          ],
                        ),
                      ),
              )
            : Stack(
                alignment: Alignment.center,
                children: [
                  FittedBox(
                    fit: BoxFit.cover,
                    clipBehavior: Clip.hardEdge,
                    child: SizedBox(
                      width: c.value.size.width,
                      height: c.value.size.height,
                      child: VideoPlayer(c),
                    ),
                  ),
                  if (!c.value.isPlaying)
                    const Icon(Icons.play_circle_fill_rounded,
                        color: Colors.white70, size: 30),
                  if (widget.sound)
                    Positioned(
                      right: 8,
                      bottom: 8,
                      child: GestureDetector(
                        onTap: () {
                          setState(() => _on = !_on);
                          unawaited(c.setVolume(_on ? 1 : 0));
                        },
                        child: Container(
                          padding: const EdgeInsets.all(7),
                          decoration: const BoxDecoration(
                              color: Colors.black54, shape: BoxShape.circle),
                          child: Icon(
                              _on
                                  ? Icons.volume_up_rounded
                                  : Icons.volume_off_rounded,
                              color: Colors.white,
                              size: 20),
                        ),
                      ),
                    ),
                ],
              ),
      ),
    );
  }
}
