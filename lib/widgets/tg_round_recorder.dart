// lib/widgets/tg_round_recorder.dart — DUMALOQ VIDEO YOZISH OYNASI.
//
// TALAB (foydalanuvchi): "dumaloq video yuborish umuman Telegram'nikidek
// emas — Cherrygram qanday ishlasa, shunga qarab tuzat".
//
// Cherrygram / Telegram `InstantCameraView` qiymatlari:
//   * chat ustida qoraygan (alpha 40/255) va xiralashgan fon;
//   * kamera doirasi — ekran qisqa tomonining ~92% i
//     (`roundPlayingMessageSize`);
//   * ochilishda doira 0.1 dan 1 gacha kattalashib, pastdan ko'tariladi
//     va paydo bo'ladi;
//   * doira atrofida 8 dp tashqarida OQ, 3 dp qalinlikdagi progress yoyi
//     (tepadan soat yo'nalishida, 60 soniya);
//   * doira ostida — kamerani almashtirish tugmasi (xira "tabletka"da);
//   * yuborilganda doira 0.1 gacha kichrayib, chap pastga — xabar
//     tomon "uchib" ketadi; bekor qilinganda shunchaki yo'qoladi.

import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:camera/camera.dart';
import 'package:flutter/material.dart';

class TgRoundOverlay extends StatefulWidget {
  final CameraController? camera;

  /// Yozilmoqda (doira ko'rinadi).
  final bool active;

  /// Yopilish sababi: `true` — yuborildi (doira xabar tomon uchadi).
  final bool sent;
  final Duration length;
  final Duration max;
  final VoidCallback? onSwitchCamera;

  const TgRoundOverlay({
    super.key,
    required this.camera,
    required this.active,
    required this.length,
    required this.max,
    this.sent = false,
    this.onSwitchCamera,
  });

  @override
  State<TgRoundOverlay> createState() => _TgRoundOverlayState();
}

class _TgRoundOverlayState extends State<TgRoundOverlay>
    with SingleTickerProviderStateMixin {
  late final AnimationController _a = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 220),
  );
  CameraController? _shown;
  bool _sent = false;

  @override
  void initState() {
    super.initState();
    if (widget.active) _open();
  }

  @override
  void didUpdateWidget(TgRoundOverlay old) {
    super.didUpdateWidget(old);
    if (widget.active && !old.active) _open();
    if (!widget.active && old.active) {
      _sent = widget.sent;
      _a.reverse();
    }
    if (widget.camera != null) _shown = widget.camera;
  }

  void _open() {
    _sent = false;
    _shown = widget.camera;
    _a.forward(from: 0);
  }

  @override
  void dispose() {
    _a.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _a,
      builder: (context, _) {
        final t = Curves.easeOutCubic.transform(_a.value);
        if (t <= 0.001) return const SizedBox.shrink();
        final size = MediaQuery.sizeOf(context);
        final side = math.min(size.width, size.height) * 0.923 - 16;
        final c = _shown;
        final progress =
            (widget.length.inMilliseconds / widget.max.inMilliseconds)
                .clamp(0.0, 1.0);
        // Yopilishda: yuborilgan bo'lsa — chap pastga uchadi.
        final closing = _a.status == AnimationStatus.reverse;
        final scale = 0.1 + 0.9 * t;
        final dx = closing && _sent ? -(size.width / 2 - 40) * (1 - t) : 0.0;
        final dy = (1 - t) * size.height / 4;
        return Positioned.fill(
          child: IgnorePointer(
            ignoring: !widget.active,
            child: Stack(
              children: [
                // Xira va qoraygan fon.
                Positioned.fill(
                  child: BackdropFilter(
                    filter: ui.ImageFilter.blur(
                        sigmaX: 12 * t, sigmaY: 12 * t),
                    child: Container(
                        color: Colors.black.withValues(alpha: 0.16 + 0.2 * t)),
                  ),
                ),
                Align(
                  alignment: const Alignment(0, -0.2),
                  child: Transform.translate(
                    offset: Offset(dx, dy),
                    child: Opacity(
                      opacity: t,
                      child: Transform.scale(
                        scale: scale,
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            SizedBox(
                              width: side + 22,
                              height: side + 22,
                              child: CustomPaint(
                                foregroundPainter: _Arc(progress),
                                child: Center(
                                  child: ClipOval(
                                    child: Container(
                                      width: side,
                                      height: side,
                                      color: Colors.black,
                                      child: c != null && c.value.isInitialized
                                          ? FittedBox(
                                              fit: BoxFit.cover,
                                              child: SizedBox(
                                                width: c.value.previewSize
                                                        ?.height ??
                                                    side,
                                                height: c.value.previewSize
                                                        ?.width ??
                                                    side,
                                                child: CameraPreview(c),
                                              ),
                                            )
                                          : null,
                                    ),
                                  ),
                                ),
                              ),
                            ),
                            const SizedBox(height: 18),
                            // Kamerani almashtirish (Telegram'dagi
                            // `buttonsLayout`).
                            GestureDetector(
                              onTap: widget.onSwitchCamera,
                              child: ClipRRect(
                                borderRadius: BorderRadius.circular(21),
                                child: BackdropFilter(
                                  filter:
                                      ui.ImageFilter.blur(sigmaX: 8, sigmaY: 8),
                                  child: Container(
                                    width: 42,
                                    height: 42,
                                    color: Colors.white.withValues(alpha: 0.14),
                                    child: const Icon(
                                        Icons.flip_camera_android_rounded,
                                        color: Colors.white,
                                        size: 24),
                                  ),
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

/// Doira atrofidagi oq progress yoyi (tepadan, soat yo'nalishida).
class _Arc extends CustomPainter {
  final double p;
  _Arc(this.p);

  @override
  void paint(Canvas canvas, Size s) {
    if (p <= 0) return;
    final paint = Paint()
      ..color = Colors.white
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3
      ..strokeCap = StrokeCap.round;
    canvas.drawArc(Offset.zero & s, -math.pi / 2, 2 * math.pi * p, false,
        paint);
  }

  @override
  bool shouldRepaint(_Arc old) => old.p != p;
}
