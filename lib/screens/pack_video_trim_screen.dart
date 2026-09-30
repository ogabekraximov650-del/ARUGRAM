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

class _PackVideoTrimScreenState extends State<PackVideoTrimScreen> {
  VideoPlayerController? _c;
  double _dur = 0;
  double _a = 0;
  double _b = 0;
  String? _error;

  int get _maxMs => packMaxSeconds(widget.kind) * 1000;

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
    _a = widget.end > widget.start ? widget.start.toDouble() : 0;
    _b = widget.end > widget.start
        ? widget.end.toDouble()
        : (_dur < _maxMs ? _dur : _maxMs.toDouble());
    _c = c;
    c.addListener(_loop);
    unawaited(c.seekTo(Duration(milliseconds: _a.round())));
    unawaited(c.play());
    setState(() {});
  }

  /// Bo'lak oxiriga yetsa — boshiga qaytadi (aylanib turadi).
  void _loop() {
    final c = _c;
    if (c == null || !c.value.isInitialized) return;
    if (c.value.position.inMilliseconds >= _b - 30) {
      unawaited(c.seekTo(Duration(milliseconds: _a.round())));
    }
  }

  @override
  void dispose() {
    _c?.removeListener(_loop);
    _c?.dispose();
    super.dispose();
  }

  void _onRange(RangeValues v) {
    var a = v.start, b = v.end;
    // Tur bo'yicha eng uzun bo'lakdan oshmasin: siljitilgan chetdan
    // qarama-qarshi chet suriladi.
    if (b - a > _maxMs) {
      if (a != _a) {
        b = a + _maxMs;
      } else {
        a = b - _maxMs;
      }
    }
    final moveStart = a != _a;
    setState(() {
      _a = a;
      _b = b;
    });
    unawaited(_c?.seekTo(Duration(milliseconds: (moveStart ? a : (b - 800 < a ? a : b - 800)).round())));
  }

  @override
  Widget build(BuildContext context) {
    final c = _c;
    final len = (_b - _a).round();
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          title: const Text('Videoni kesish',
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
                            onTap: () => c.value.isPlaying ? c.pause() : c.play(),
                            child: AspectRatio(
                              aspectRatio: c.value.aspectRatio,
                              child: VideoPlayer(c),
                            ),
                          ),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                        child: Column(
                          children: [
                            RangeSlider(
                              min: 0,
                              max: _dur,
                              values: RangeValues(_a, _b),
                              activeColor: AppColors.accent,
                              onChanged: _onRange,
                            ),
                            Text(
                              '${fmtMs(_a.round())} – ${fmtMs(_b.round())}   ·   '
                              '${(len / 1000).toStringAsFixed(1)} s '
                              '(ko\'pi bilan ${packMaxSeconds(widget.kind)} s)',
                              style: const TextStyle(
                                  color: Colors.white70, fontSize: 13.5),
                            ),
                            const SizedBox(height: 6),
                            const Text(
                              'Faqat tanlangan bo\'lak qoladi, ovoz olib tashlanadi.',
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
