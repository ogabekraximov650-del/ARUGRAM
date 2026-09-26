// lib/widgets/tg_record_button.dart — TELEGRAM'DAGIDEK YUBORISH /
// OVOZ / DUMALOQ VIDEO TUGMASI.
//
// TALAB (foydalanuvchi): "support chatga ovozli xabar va dumaloq
// xabar yuboradigan oynani qo'sh — huddi Telegram'niki bilan bir
// xil bo'lsin".
//
// Telegram Android'dagi xatti-harakat:
//   * matn yozilgan bo'lsa — ➤ (yuborish);
//   * bo'sh bo'lsa — 🎤 yoki 📷; QISQA bosish ular orasida almashtiradi;
//   * BOSIB TURISH — yozish boshlanadi (tugma kattalashadi);
//     qo'yib yuborilsa — yuboriladi;
//   * chapga surilsa — bekor qilinadi;
//   * tepaga surilsa — QULFLANADI: barmoqni qo'yib yuborsa ham yozish
//     davom etadi, keyin ➤ bilan yuboriladi.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'glass.dart';

enum TgRecMode { voice, video }

class TgRecordButton extends StatefulWidget {
  final bool hasText;
  final bool busy;

  /// Yozish qulflangan (tepaga surilgan) — tugma ➤ bo'lib turadi.
  final bool locked;
  final VoidCallback onSend;

  /// Yozishni boshlaydi. `false` — boshlanmadi (ruxsat yo'q va h.k.).
  final Future<bool> Function(TgRecMode mode) onStart;

  /// Yozishni tugatadi: [send] — yuborish yoki bekor qilish.
  final void Function(bool send) onStop;
  final VoidCallback onLock;

  /// Chapga surilgan masofa (manfiy) — "bekor qilish uchun suring"
  /// yozuvi shunga qarab siljiydi.
  final ValueChanged<double> onDrag;

  const TgRecordButton({
    super.key,
    required this.hasText,
    required this.busy,
    required this.locked,
    required this.onSend,
    required this.onStart,
    required this.onStop,
    required this.onLock,
    required this.onDrag,
  });

  @override
  State<TgRecordButton> createState() => _TgRecordButtonState();
}

class _TgRecordButtonState extends State<TgRecordButton> {
  /// Oxirgi tanlangan rejim (ekranlar orasida saqlanadi).
  static TgRecMode _lastMode = TgRecMode.voice;

  TgRecMode _mode = _lastMode;
  Timer? _hold;
  bool _holding = false;

  /// Barmoq hozir tugmada.
  bool _pressed = false;
  Offset _start = Offset.zero;

  // Telegram Android (`ChatActivityEnterView`) qiymatlari:
  //   * yozish bosilgandan 150 ms keyin boshlanadi;
  //   * `distCanMove` = ekran kengligining 35% (eng ko'pi 140 dp),
  //     surish "0.7 dan kam" bo'lganda bekor qilinadi, ya'ni
  //     `distCanMove * 0.3` chapga surilganda;
  //   * tepaga 57 dp surilsa — qulflanadi.
  static const _lockAt = -57.0;
  double _cancelAt(BuildContext c) =>
      -(MediaQuery.sizeOf(c).width * 0.35).clamp(0.0, 140.0) * 0.3;

  @override
  void dispose() {
    _hold?.cancel();
    super.dispose();
  }

  void _down(PointerDownEvent e) {
    if (widget.busy) return;
    if (widget.hasText || widget.locked) return; // bosish — `_up` da
    _start = e.position;
    _pressed = true;
    _hold?.cancel();
    _hold = Timer(const Duration(milliseconds: 150), () async {
      _hold = null;
      final ok = await widget.onStart(_mode);
      if (!mounted) return;
      // Yozish boshlanguncha (kamera ochilguncha) barmoq qo'yib
      // yuborilgan — yozuv osilib qolmasin.
      if (ok && !_pressed && !widget.locked) {
        widget.onStop(false);
        return;
      }
      setState(() => _holding = ok);
    });
  }

  void _move(PointerMoveEvent e) {
    if (!_holding || widget.locked) return;
    final d = e.position - _start;
    widget.onDrag(d.dx.clamp(-200.0, 0.0));
    if (d.dx < _cancelAt(context)) {
      setState(() => _holding = false);
      widget.onDrag(0);
      widget.onStop(false);
    } else if (d.dy < _lockAt) {
      setState(() => _holding = false);
      widget.onDrag(0);
      widget.onLock();
    }
  }

  void _up(PointerUpEvent e) {
    _pressed = false;
    if (widget.busy) return;
    if (widget.hasText) {
      widget.onSend();
      return;
    }
    if (widget.locked) {
      widget.onStop(true);
      return;
    }
    if (_hold != null) {
      // Qisqa bosish — mikrofon <-> kamera.
      _hold!.cancel();
      _hold = null;
      HapticFeedback.selectionClick();
      setState(() {
        _mode = _lastMode =
            _mode == TgRecMode.voice ? TgRecMode.video : TgRecMode.voice;
      });
      return;
    }
    if (_holding) {
      setState(() => _holding = false);
      widget.onDrag(0);
      widget.onStop(true);
    }
  }

  void _cancel(PointerCancelEvent e) {
    _pressed = false;
    _hold?.cancel();
    _hold = null;
    if (_holding) {
      setState(() => _holding = false);
      widget.onDrag(0);
      widget.onStop(false);
    }
  }

  @override
  Widget build(BuildContext context) {
    // Telegram'dagidek: matn bo'lsa ➤, aks holda 🎤 yoki dumaloq
    // kamera belgisi. Almashganda eski belgi kichrayib, burilib
    // yo'qoladi, yangisi kattalashib chiqadi.
    final key = widget.hasText || widget.locked
        ? 'send'
        : (_mode == TgRecMode.voice ? 'mic' : 'video');
    final Widget glyph = switch (key) {
      'send' => const Icon(Icons.send_rounded, size: 22, color: Colors.white),
      'mic' => const Icon(Icons.mic_rounded, size: 24, color: Colors.white),
      _ => const _RoundVideoGlyph(),
    };
    return Listener(
      onPointerDown: _down,
      onPointerMove: _move,
      onPointerUp: _up,
      onPointerCancel: _cancel,
      behavior: HitTestBehavior.opaque,
      child: Stack(
        clipBehavior: Clip.none,
        alignment: Alignment.center,
        children: [
          // Tepaga surib qulflash belgisi (yozish paytida).
          if (_holding)
            const Positioned(
              bottom: 76,
              child: _LockHint(),
            ),
          // Yozish paytida atrofda "nafas oladigan" halqa (Telegram'da
          // ovoz balandligiga qarab kattalashadi).
          if (_holding) const _Halo(),
          AnimatedScale(
            scale: _holding ? 1.9 : 1,
            duration: const Duration(milliseconds: 220),
            curve: _holding ? Curves.easeOutBack : Curves.easeOut,
            child: Container(
              width: 44,
              height: 44,
              decoration: const BoxDecoration(
                shape: BoxShape.circle,
                color: AppColors.accent,
              ),
              child: widget.busy
                  ? const Padding(
                      padding: EdgeInsets.all(12),
                      child: CircularProgressIndicator(
                          strokeWidth: 2, color: Colors.white),
                    )
                  : AnimatedSwitcher(
                      duration: const Duration(milliseconds: 260),
                      switchInCurve: Curves.easeOutBack,
                      switchOutCurve: Curves.easeIn,
                      transitionBuilder: (c, a) => FadeTransition(
                        opacity: a,
                        child: ScaleTransition(
                          scale: Tween(begin: 0.2, end: 1.0).animate(a),
                          child: RotationTransition(
                            turns: Tween(begin: -0.12, end: 0.0).animate(a),
                            child: c,
                          ),
                        ),
                      ),
                      child: KeyedSubtree(key: ValueKey(key), child: glyph),
                    ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Telegram'ning dumaloq video belgisi: halqa ichida kamera.
class _RoundVideoGlyph extends StatelessWidget {
  const _RoundVideoGlyph();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 24,
      height: 24,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        border: Border.all(color: Colors.white, width: 2),
      ),
      child: const Icon(Icons.videocam_rounded, size: 13, color: Colors.white),
    );
  }
}

/// Yozish paytidagi yumshoq, to'lqinlanadigan halqa.
class _Halo extends StatefulWidget {
  const _Halo();

  @override
  State<_Halo> createState() => _HaloState();
}

class _HaloState extends State<_Halo> with SingleTickerProviderStateMixin {
  late final AnimationController _a = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 900))
    ..repeat(reverse: true);

  @override
  void dispose() {
    _a.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      child: AnimatedBuilder(
        animation: _a,
        builder: (_, __) => Transform.scale(
          scale: 2.2 + 0.35 * Curves.easeInOut.transform(_a.value),
          child: Container(
            width: 44,
            height: 44,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: AppColors.accent.withValues(alpha: 0.22),
            ),
          ),
        ),
      ),
    );
  }
}

class _LockHint extends StatelessWidget {
  const _LockHint();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 36,
      padding: const EdgeInsets.symmetric(vertical: 8),
      decoration: BoxDecoration(
        color: const Color(0xEE2A2F36),
        borderRadius: BorderRadius.circular(18),
      ),
      child: const Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.lock_open_rounded, size: 18, color: Colors.white70),
          SizedBox(height: 2),
          Icon(Icons.keyboard_arrow_up_rounded,
              size: 18, color: Colors.white54),
        ],
      ),
    );
  }
}
