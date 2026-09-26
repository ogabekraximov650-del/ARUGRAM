// lib/widgets/tg_round_recorder.dart — DUMALOQ VIDEO YOZISH OYNASI.
//
// TALAB (foydalanuvchi): "dumaloq video yuborish umuman Telegram'nikidek
// emas — Cherrygram qanday ishlasa, shunga qarab tuzat".
//
// Cherrygram / Telegram `InstantCameraView` qiymatlari:
//   * chat ustida qoraygan (alpha 40/255) va xiralashgan fon;
//   * kamera doirasi — ekran qisqa tomonining ~92% i
//     (`roundPlayingMessageSize`);
//   * ochilishda doira 0.1 dan 1 gacha kattalashib, yarim balandlikdan
//     pastdan ko'tariladi va paydo bo'ladi (180 ms, `Decelerate`);
//   * doira atrofida 8 dp tashqarida OQ, 3 dp qalinlikdagi progress yoyi
//     (tepadan soat yo'nalishida, 60 soniya);
//   * pastda chapda (`buttonsLayout`, 56 dp, shisha fon) — kamerani
//     almashtirish (bosilganda aylanadi) va chiroq (old kamerada ekranning
//     o'zi iliq oq yonadi — `FlashViews`);
//   * doira faqat xabarlar ustida: pastdagi yozish paneli ko'rinib turadi;
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

  /// Chiroq yoqilgan (old kamerada ekran oq yonadi).
  final bool flash;
  final VoidCallback? onFlash;

  const TgRoundOverlay({
    super.key,
    required this.camera,
    required this.active,
    required this.length,
    required this.max,
    this.sent = false,
    this.onSwitchCamera,
    this.flash = false,
    this.onFlash,
  });

  @override
  State<TgRoundOverlay> createState() => _TgRoundOverlayState();
}

class _TgRoundOverlayState extends State<TgRoundOverlay>
    with SingleTickerProviderStateMixin {
  late final AnimationController _a = AnimationController(
    vsync: this,
    // `InstantCameraView.animate`: 180 ms, `DecelerateInterpolator`.
    duration: const Duration(milliseconds: 180),
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
        final t = Curves.decelerate.transform(_a.value);
        if (t <= 0.001) return const SizedBox.shrink();
        return Positioned.fill(child: LayoutBuilder(builder: (context, box) {
          final screen = MediaQuery.sizeOf(context);
          // `roundPlayingMessageSize`: ekran qisqa tomonining ~92% i.
          final side = (math.min(screen.width, screen.height) * 0.923 - 16)
              .clamp(120.0, box.maxHeight - 80);
          final c = _shown;
          final progress =
              (widget.length.inMilliseconds / widget.max.inMilliseconds)
                  .clamp(0.0, 1.0);
          final closing = _a.status == AnimationStatus.reverse;
          final scale = 0.1 + 0.9 * t;
          // Yuborilganda chapga (`dp(24) - width / 2`) — xabar tomon.
          final dx = closing && _sent ? (24 - box.maxWidth / 2) * (1 - t) : 0.0;
          // `animationTranslationY`: yarim balandlikdan 0 gacha.
          final dy = (1 - t) * box.maxHeight / 2;
          final front =
              c?.description.lensDirection == CameraLensDirection.front;
          return IgnorePointer(
            ignoring: !widget.active,
            child: Stack(
              children: [
                // Xira va qoraygan fon.
                Positioned.fill(
                  child: BackdropFilter(
                    filter:
                        ui.ImageFilter.blur(sigmaX: 12 * t, sigmaY: 12 * t),
                    child: Container(
                        color: Colors.black.withValues(alpha: 0.16 + 0.2 * t)),
                  ),
                ),
                // Old kamera chirog'i — ekranning o'zi oq yonadi
                // (`FlashViews`: iliq oq, doira atrofida).
                if (widget.flash && front)
                  Positioned.fill(
                    child: IgnorePointer(
                      child: Container(
                          color: const Color(0xFFFFF4E0)
                              .withValues(alpha: 0.9 * t)),
                    ),
                  ),
                Center(
                  child: Transform.translate(
                    offset: Offset(dx, dy),
                    child: Opacity(
                      opacity: t,
                      child: Transform.scale(
                        scale: scale,
                        child: SizedBox(
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
                                            width: c.value.previewSize?.height ??
                                                side,
                                            height:
                                                c.value.previewSize?.width ??
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
                      ),
                    ),
                  ),
                ),
                // Tugmalar (`buttonsLayout`): pastda chapda, balandligi 56,
                // shisha fonda — kamerani almashtirish va chiroq (44 dp).
                Positioned(
                  left: 8,
                  bottom: 8,
                  child: Opacity(
                    opacity: t,
                    child: ClipRRect(
                      borderRadius: BorderRadius.circular(28),
                      child: BackdropFilter(
                        filter: ui.ImageFilter.blur(sigmaX: 16, sigmaY: 16),
                        child: Container(
                          height: 56,
                          padding: const EdgeInsets.all(6),
                          color: Colors.white.withValues(alpha: 0.14),
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              _RoundBtn(
                                icon: Icons.flip_camera_android_rounded,
                                onTap: widget.onSwitchCamera,
                              ),
                              _RoundBtn(
                                icon: widget.flash
                                    ? Icons.flash_on_rounded
                                    : Icons.flash_off_rounded,
                                onTap: widget.onFlash,
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          );
        }));
      },
    );
  }
}

/// `switchCameraButton` / `flashButton` — 44 dp.
class _RoundBtn extends StatefulWidget {
  final IconData icon;
  final VoidCallback? onTap;
  const _RoundBtn({required this.icon, this.onTap});

  @override
  State<_RoundBtn> createState() => _RoundBtnState();
}

class _RoundBtnState extends State<_RoundBtn> {
  int _turns = 0;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () {
        // Kamera almashtirilganda belgi aylanadi (`switchCameraDrawable`).
        if (widget.icon == Icons.flip_camera_android_rounded) {
          setState(() => _turns++);
        }
        widget.onTap?.call();
      },
      child: SizedBox(
        width: 44,
        height: 44,
        child: AnimatedRotation(
          turns: _turns / 2,
          duration: const Duration(milliseconds: 580),
          curve: Curves.easeOutCubic,
          child: AnimatedSwitcher(
            duration: const Duration(milliseconds: 180),
            child: Icon(widget.icon,
                key: ValueKey(widget.icon), color: Colors.white, size: 24),
          ),
        ),
      ),
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
