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
import 'package:video_player/video_player.dart';

import '../services/pack_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';

/// Tur bo'yicha eng uzun bo'lak (soniya) — `tool/packs/arunorm.py` bilan bir xil.
int packMaxSeconds(String kind) => switch (kind) {
      PackKind.emoji => 5,
      PackKind.gif => 15,
      _ => 8,
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
  const PackVideoTrimScreen({
    super.key,
    required this.path,
    required this.kind,
    this.start = 0,
    this.end = 0,
  });

  @override
  State<PackVideoTrimScreen> createState() => _PackVideoTrimScreenState();
}

enum _Drag { none, left, right, body }

class _PackVideoTrimScreenState extends State<PackVideoTrimScreen> {
  static const double _minMs = 500;
  static const double _handleW = 16;
  static const int _frameCount = 8;

  VideoPlayerController? _c;
  double _dur = 0;
  double _a = 0;
  double _b = 0;
  double _pos = 0;
  String? _error;
  List<Uint8List?> _frames = const [];
  _Drag _drag = _Drag.none;

  int get _maxMs => packMaxSeconds(widget.kind) * 1000;
  double get _maxSel => _dur < _maxMs ? _dur : _maxMs.toDouble();
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
    c.setVolume(0);
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
    if ((x - ax).abs() <= 26 && (x - ax).abs() <= (x - bx).abs()) {
      return _Drag.left;
    }
    if ((x - bx).abs() <= 26) return _Drag.right;
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

  Widget _timeline(double w) {
    final ax = _a / _dur * w;
    final bx = _b / _dur * w;
    final px = (_pos.clamp(_a, _b) / _dur * w).clamp(ax, bx).toDouble();
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onHorizontalDragStart: (d) => _onStart(d, w),
      onHorizontalDragUpdate: (d) => _onUpdate(d, w),
      onHorizontalDragEnd: (_) => _onEnd(),
      onHorizontalDragCancel: _onEnd,
      child: SizedBox(
        height: 64,
        width: w,
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            // kadrlar
            Positioned.fill(
              child: ClipRRect(
                borderRadius: BorderRadius.circular(6),
                child: Row(
                  children: [
                    for (var i = 0; i < _frameCount; i++)
                      Expanded(
                        child: i < _frames.length && _frames[i] != null
                            ? Image.memory(_frames[i]!,
                                fit: BoxFit.cover,
                                height: 64,
                                gaplessPlayback: true)
                            : Container(color: Colors.white12),
                      ),
                  ],
                ),
              ),
            ),
            // tanlanmagan joylar — xira
            Positioned(
                left: 0,
                width: ax,
                top: 0,
                bottom: 0,
                child: Container(color: Colors.black.withValues(alpha: 0.65))),
            Positioned(
                left: bx,
                right: 0,
                top: 0,
                bottom: 0,
                child: Container(color: Colors.black.withValues(alpha: 0.65))),
            // tanlangan oyna: ramka va dastalar
            Positioned(
              left: ax - _handleW / 2,
              width: bx - ax + _handleW,
              top: -3,
              bottom: -3,
              child: Container(
                decoration: BoxDecoration(
                  border: Border.symmetric(
                      horizontal:
                          BorderSide(color: AppColors.accent, width: 3)),
                ),
              ),
            ),
            for (final x in [ax, bx])
              Positioned(
                left: x - _handleW / 2,
                width: _handleW,
                top: -3,
                bottom: -3,
                child: Container(
                  decoration: BoxDecoration(
                    color: AppColors.accent,
                    borderRadius: BorderRadius.circular(5),
                  ),
                  child: const Center(
                    child: SizedBox(
                      width: 3,
                      height: 22,
                      child: DecoratedBox(
                          decoration: BoxDecoration(
                              color: Colors.white,
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
              top: -6,
              bottom: -6,
              child: const DecoratedBox(
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.all(Radius.circular(2)),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final c = _c;
    final len = (_b - _a).round();
    final maxS = packMaxSeconds(widget.kind);
    final atMax = _b - _a >= _maxSel - 1;
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          title: const Text('Videoni tahrirlash',
              style: TextStyle(color: Colors.white, fontSize: 18)),
          actions: [
            TextButton(
              onPressed: c == null
                  ? null
                  : () => Navigator.of(context).pop((_a.round(), _b.round())),
              child: const Text('Tayyor'),
            ),
          ],
        ),
        body: _error != null
            ? Center(
                child: Text(_error!,
                    style: const TextStyle(color: AppColors.danger)))
            : c == null
                ? const Center(
                    child: CircularProgressIndicator(
                        strokeWidth: 2.4, color: Colors.white54))
                : Column(
                    children: [
                      Expanded(
                        child: Center(
                          child: GestureDetector(
                            onTap: () =>
                                c.value.isPlaying ? c.pause() : c.play(),
                            child: AspectRatio(
                              aspectRatio: c.value.aspectRatio,
                              child: VideoPlayer(c),
                            ),
                          ),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.fromLTRB(24, 8, 24, 0),
                        child: Column(
                          children: [
                            Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                Text(
                                  '${(len / 1000).toStringAsFixed(1)} s',
                                  style: TextStyle(
                                      color: atMax
                                          ? AppColors.accent
                                          : Colors.white,
                                      fontSize: 20,
                                      fontWeight: FontWeight.w700),
                                ),
                                const SizedBox(width: 8),
                                Text('· eng ko\'pi $maxS s',
                                    style: const TextStyle(
                                        color: Colors.white54,
                                        fontSize: 14)),
                              ],
                            ),
                            const SizedBox(height: 14),
                            LayoutBuilder(
                              builder: (_, cs) => _timeline(cs.maxWidth),
                            ),
                            const SizedBox(height: 8),
                            Row(
                              mainAxisAlignment:
                                  MainAxisAlignment.spaceBetween,
                              children: [
                                Text(fmtMs(_a.round()),
                                    style: const TextStyle(
                                        color: Colors.white54, fontSize: 12)),
                                Text(fmtMs(_dur.round()),
                                    style: const TextStyle(
                                        color: Colors.white54, fontSize: 12)),
                              ],
                            ),
                            const SizedBox(height: 10),
                            const Text(
                              'Chetlarini yoki oynani sudrang. Faqat tanlangan '
                              'bo\'lak qoladi, ovoz olib tashlanadi.',
                              textAlign: TextAlign.center,
                              style: TextStyle(
                                  color: Colors.white38, fontSize: 12),
                            ),
                            const SizedBox(height: 18),
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
  const PackVideoPreview({super.key, required this.path, this.size = 96});

  @override
  State<PackVideoPreview> createState() => _PackVideoPreviewState();
}

class _PackVideoPreviewState extends State<PackVideoPreview> {
  VideoPlayerController? _c;

  @override
  void initState() {
    super.initState();
    unawaited(_init());
  }

  Future<void> _init() async {
    final c = VideoPlayerController.file(File(widget.path));
    try {
      await c.initialize();
      await c.setVolume(0);
      await c.setLooping(true);
    } catch (_) {
      await c.dispose();
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
            ? const Center(
                child: Icon(Icons.movie_outlined, color: Colors.white38))
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
                ],
              ),
      ),
    );
  }
}
