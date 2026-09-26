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
    _hold?.cancel();
    _hold = Timer(const Duration(milliseconds: 150), () async {
      _hold = null;
      final ok = await widget.onStart(_mode);
      if (!mounted) return;
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
    final icon = widget.hasText || widget.locked
        ? Icons.send_rounded
        : (_mode == TgRecMode.voice
            ? Icons.mic_rounded
            : Icons.radio_button_checked_rounded);
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
              bottom: 64,
              child: _LockHint(),
            ),
          AnimatedScale(
            scale: _holding ? 1.55 : 1,
            duration: const Duration(milliseconds: 160),
            curve: Curves.easeOut,
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
                      duration: const Duration(milliseconds: 160),
                      transitionBuilder: (c, a) =>
                          ScaleTransition(scale: a, child: c),
                      child: Icon(icon,
                          key: ValueKey(icon), size: 22, color: Colors.white),
                    ),
            ),
          ),
        ],
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
