// lib/widgets/tg_record_button.dart — TELEGRAM'DAGIDEK YUBORISH /
// OVOZ / DUMALOQ VIDEO TUGMASI.
//
// TALAB (foydalanuvchi): "Dumaloq video olish va ovozli xabar
// tugmalarining ikonkasi va ustiga bosgandagi animatsiyasini xuddi
// Telegram'nikidek qil".
//
// Manba: Cherrygram `ChatActivityEnterView` (`RecordCircle`,
// `ControlsView`), `BlobDrawable`, `WaveDrawable`,
// `ChatActivityEnterViewAnimatedIconView` (`voice_and_video.json`):
//
//   * bo'sh holatda 🎤 yoki 📹 — Lottie: 🎤 = 30-kadr, 📹 = 0/60-kadr;
//     QISQA bosish ular orasida aylanib almashtiradi;
//   * matn yozilgan bo'lsa — ➤;
//   * BOSIB TURISH (150 ms) — yozish: tugma o'rnida radiusi
//     41 + 30 × ovoz balandligi bo'lgan doira ochiladi (0 → 1.1 → 0.9
//     → 1 "sakrash"), atrofida ikki qatlam "pufak" to'lqin
//     (katta: 50..57 dp, 30% shaffof; kichik: 47..56 dp, 15%) —
//     ovoz qancha baland bo'lsa shuncha tez va katta tebranadi;
//   * doira ustida (60 dp yuqorida) qulf "tabletkasi" (36 × 50, tepaga
//     o'q bilan, sekin tebranadi); tepaga 57 dp surilsa qulflanadi —
//     doira ➤ ga aylanadi. Qulflangach barmoqni qo'yib yuborish hech
//     narsa qilmaydi — faqat ➤ bosilganda yuboriladi;
//   * chapga surilsa doira barmoq bilan siljiydi va 0.7 gacha
//     kichrayadi; `distCanMove` ning to'liq masofasiga surilsa (yoki
//     0.45 dan kam holatda qo'yib yuborilsa) — bekor. Chapga 30% dan
//     ko'p surilgan bo'lsa qulflanmaydi (`slideToCancelProgress < 0.7`);
//   * qisqa bosishda 🎤 <-> 📹 almashadi va tepada "bosib turing"
//     maslahati 2 soniya ko'rinadi (`HoldToAudio` / `HoldToVideo`).

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:lottie/lottie.dart';

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

  /// Ovoz balandligi 0..1 (Telegram'dagi `amplitude / 1800`).
  final ValueListenable<double>? amplitude;

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
    this.amplitude,
  });

  @override
  State<TgRecordButton> createState() => _TgRecordButtonState();
}

class _TgRecordButtonState extends State<TgRecordButton>
    with TickerProviderStateMixin {
  /// Oxirgi tanlangan rejim (ekranlar orasida saqlanadi).
  static TgRecMode _lastMode = TgRecMode.voice;

  static const double _size = 48;

  TgRecMode _mode = _lastMode;
  Timer? _hold;
  bool _holding = false;

  /// Barmoq hozir tugmada.
  bool _pressed = false;

  /// Barmoq bosilgan nuqta — surish shundan o'lchanadi.
  Offset _start = Offset.zero;

  /// Yozish boshlanmoqda (kamera/mikrofon ochilyapti) — doira allaqachon
  /// ko'rinadi, barmoq bilan surish va qulflash shu paytda ham ishlaydi.
  bool _starting = false;

  /// Boshlanish paytida bekor qilindi — boshlangach darhol to'xtatiladi.
  bool _abort = false;
  double _dx = 0;
  double _dy = 0;

  /// Shu bosishda qulflandi — barmoq ko'tarilganda yuborilmaydi.
  bool _lockedNow = false;

  /// "Bosib turing" maslahati.
  final _hint = OverlayPortalController();
  final _link = LayerLink();
  Timer? _hintTimer;

  /// 🎤 <-> 📹 belgisi (`voice_and_video.json`, 60 kadr).
  late final AnimationController _icon = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1000),
      // Telegram (RLottie) tizimdagi "animatsiyalarni o'chirish"ga
      // qaramaydi — belgi doim aylanib almashadi.
      animationBehavior: AnimationBehavior.preserve,
      value: _mode == TgRecMode.voice ? 0.5 : 0);

  /// Doiraning ochilishi (`scale`, 0..1).
  late final AnimationController _enter = AnimationController(
      vsync: this,
      animationBehavior: AnimationBehavior.preserve,
      duration: const Duration(milliseconds: 360));

  // Telegram Android (`ChatActivityEnterView`) qiymatlari:
  //   * yozish bosilgandan 150 ms keyin boshlanadi;
  //   * `distCanMove` = ekran kengligining 35% (eng ko'pi 140 dp),
  //     surish "0.7 dan kam" bo'lganda bekor qilinadi, ya'ni
  //     `distCanMove * 0.3` chapga surilganda;
  //   * tepaga 57 dp surilsa — qulflanadi.
  static const _lockAt = 57.0;
  double _distCanMove(BuildContext c) =>
      (MediaQuery.sizeOf(c).width * 0.35).clamp(0.0, 140.0);

  @override
  void dispose() {
    _hold?.cancel();
    _hintTimer?.cancel();
    _icon.dispose();
    _enter.dispose();
    super.dispose();
  }

  void _setHolding(bool v) {
    if (v == _holding) return;
    setState(() => _holding = v);
    if (v) {
      _enter.forward(from: 0);
    } else {
      _enter.value = 0;
      _dx = 0;
      _dy = 0;
    }
  }

  void _down(PointerDownEvent e) {
    if (widget.busy) return;
    if (widget.hasText || widget.locked) return; // bosish — `_up` da
    _start = e.position;
    _lockedNow = false;
    _abort = false;
    _pressed = true;
    _hold?.cancel();
    _hold = Timer(const Duration(milliseconds: 150), () async {
      _hold = null;
      // Telegram'dagidek doira DARHOL ochiladi — kamera ochilishini
      // kutmaydi (ilgari shu paytdagi surish/qulflash yo'qolardi).
      _starting = true;
      _setHolding(true);
      HapticFeedback.lightImpact();
      final ok = await widget.onStart(_mode);
      _starting = false;
      if (!mounted) return;
      if (!ok) {
        _setHolding(false);
        widget.onDrag(0);
        if (widget.locked || _lockedNow) widget.onStop(false);
        _lockedNow = false;
        return;
      }
      // Boshlanguncha bekor qilingan yoki barmoq qo'yib yuborilgan
      // (qulflanmagan) — yozuv osilib qolmasin.
      if (_abort || (!_pressed && !widget.locked && !_lockedNow)) {
        _abort = false;
        _setHolding(false);
        widget.onDrag(0);
        widget.onStop(false);
      }
    });
  }

  void _move(PointerMoveEvent e) {
    if (!_holding || widget.locked) return;
    // Masofa barmoq BOSILGAN joydan o'lchanadi — yozish boshlanguncha
    // qilingan harakat ham hisobga kiradi.
    final d = e.position - _start;
    final dist = _distCanMove(context);
    final slide = (1 + d.dx / dist).clamp(0.0, 1.0);
    // `setLockTranslation`: chapga 30% dan ko'p surilgan bo'lsa
    // qulflanmaydi; aks holda tepaga 57 dp — qulf.
    if (slide >= 0.7 && -d.dy >= _lockAt) {
      HapticFeedback.mediumImpact();
      _lockedNow = true;
      _setHolding(false);
      widget.onDrag(0);
      widget.onLock();
      return;
    }
    setState(() {
      _dx = d.dx.clamp(-dist, 0.0);
      _dy = (-d.dy).clamp(0.0, _lockAt);
    });
    widget.onDrag(_dx);
    // `alpha == 0` — to'liq masofaga surildi: bekor.
    if (slide <= 0) {
      _setHolding(false);
      widget.onDrag(0);
      if (_starting) {
        _abort = true;
      } else {
        widget.onStop(false);
      }
    }
  }

  void _up(PointerUpEvent e) {
    _pressed = false;
    if (_lockedNow) {
      // Shu bosishda qulflandi — barmoq ko'tarilishi yubormaydi.
      _lockedNow = false;
      return;
    }
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
      // Qisqa bosish — 🎤 <-> 📹 (belgi aylanib almashadi).
      _hold!.cancel();
      _hold = null;
      HapticFeedback.selectionClick();
      _toggleMode();
      return;
    }
    if (_starting) {
      // Hali boshlanmagan — boshlangach darhol to'xtatiladi.
      _abort = true;
      _setHolding(false);
      widget.onDrag(0);
      return;
    }
    if (_holding) {
      // `alpha < 0.45` holatda qo'yib yuborilsa — bekor.
      final cancel = 1 + _dx / _distCanMove(context) < 0.45;
      _setHolding(false);
      widget.onDrag(0);
      widget.onStop(!cancel);
    }
  }

  void _toggleMode() {
    setState(() {
      _mode = _lastMode =
          _mode == TgRecMode.voice ? TgRecMode.video : TgRecMode.voice;
    });
    // `setState(VIDEO)`: 30 -> 60 kadr; `setState(VOICE)`: 0 -> 30.
    if (_mode == TgRecMode.video) {
      _icon.value = 0.5;
      _icon.animateTo(1, duration: const Duration(milliseconds: 500));
    } else {
      _icon.value = 0;
      _icon.animateTo(0.5, duration: const Duration(milliseconds: 500));
    }
    _showHint();
  }

  void _showHint() {
    _hintTimer?.cancel();
    if (_hint.isShowing) {
      // Matn yangilansin.
      setState(() {});
    } else {
      _hint.show();
    }
    _hintTimer = Timer(const Duration(seconds: 2), () {
      if (mounted && _hint.isShowing) _hint.hide();
    });
  }

  Widget _hintBubble(BuildContext context) {
    final text = _mode == TgRecMode.video
        ? 'Video xabar yozish uchun bosib turing.\nOvozga o\'tish uchun bosing.'
        : 'Ovozli xabar yozish uchun bosib turing.\nVideoga o\'tish uchun bosing.';
    return CompositedTransformFollower(
      link: _link,
      targetAnchor: Alignment.topRight,
      followerAnchor: Alignment.bottomRight,
      offset: const Offset(0, -6),
      child: Align(
        alignment: Alignment.bottomRight,
        child: IgnorePointer(
          child: TweenAnimationBuilder<double>(
            tween: Tween(begin: 0, end: 1),
            duration: const Duration(milliseconds: 150),
            builder: (context, v, child) => Opacity(opacity: v, child: child),
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 7),
              decoration: BoxDecoration(
                color: const Color(0xE6202226),
                borderRadius: BorderRadius.circular(6),
              ),
              child: Text(text,
                  style: const TextStyle(
                      color: Colors.white,
                      fontSize: 14,
                      decoration: TextDecoration.none,
                      fontWeight: FontWeight.w400)),
            ),
          ),
        ),
      ),
    );
  }

  void _cancel(PointerCancelEvent e) {
    _pressed = false;
    _hold?.cancel();
    _hold = null;
    if (_starting) {
      _abort = true;
      _setHolding(false);
      widget.onDrag(0);
      return;
    }
    if (_holding) {
      _setHolding(false);
      widget.onDrag(0);
      widget.onStop(false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final send = widget.hasText || widget.locked;
    final dist = _distCanMove(context);
    // `slideToCancelProgress`: 1 — joyida, 0 — bekor chegarasida.
    final slide = (1 + _dx / dist).clamp(0.0, 1.0);
    return OverlayPortal(
      controller: _hint,
      overlayChildBuilder: _hintBubble,
      child: CompositedTransformTarget(
      link: _link,
      child: Listener(
      onPointerDown: _down,
      onPointerMove: _move,
      onPointerUp: _up,
      onPointerCancel: _cancel,
      behavior: HitTestBehavior.opaque,
      child: SizedBox(
        width: _size,
        height: _size,
        child: Stack(
          clipBehavior: Clip.none,
          alignment: Alignment.center,
          children: [
            // Oddiy tugma (yozish paytida doira ostida yashiringan).
            AnimatedOpacity(
              duration: const Duration(milliseconds: 150),
              opacity: _holding ? 0 : 1,
              child: Container(
                width: _size,
                height: _size,
                decoration: const BoxDecoration(
                  shape: BoxShape.circle,
                  color: AppColors.accent,
                ),
                child: widget.busy
                    ? const Padding(
                        padding: EdgeInsets.all(13),
                        child: CircularProgressIndicator(
                            strokeWidth: 2, color: Colors.white),
                      )
                    : AnimatedSwitcher(
                        duration: const Duration(milliseconds: 200),
                        switchInCurve: Curves.easeOutBack,
                        switchOutCurve: Curves.easeIn,
                        transitionBuilder: (c, a) => FadeTransition(
                          opacity: a,
                          child: ScaleTransition(
                            scale: Tween(begin: 0.1, end: 1.0).animate(a),
                            child: c,
                          ),
                        ),
                        child: send
                            ? const Padding(
                                key: ValueKey('send'),
                                padding: EdgeInsets.only(left: 3),
                                child: Icon(Icons.send_rounded,
                                    size: 24, color: Colors.white),
                              )
                            : SizedBox(
                                key: const ValueKey('rec'),
                                width: 26,
                                height: 26,
                                child: ColorFiltered(
                                  colorFilter: const ColorFilter.mode(
                                      Colors.white, BlendMode.srcIn),
                                  child: Lottie.asset(
                                    'assets/tg_anim/voice_and_video.json',
                                    controller: _icon,
                                  ),
                                ),
                              ),
                      ),
              ),
            ),
            if (_holding)
              Positioned(
                // Doira va qulf tugmadan tashqariga chiqadi.
                left: _size / 2 - 150,
                top: _size / 2 - 230,
                width: 300,
                height: 300,
                child: IgnorePointer(
                  child: _RecordCircle(
                    enter: _enter,
                    amplitude: widget.amplitude,
                    video: _mode == TgRecMode.video,
                    slideDx: _dx,
                    slide: slide,
                    lockMove: _dy / _lockAt,
                  ),
                ),
              ),
          ],
        ),
      ),
    ),
    ),
    );
  }
}

// ══════════════════════════════════════════════════════════════
//  YOZISH DOIRASI (`RecordCircle`)
// ══════════════════════════════════════════════════════════════

class _RecordCircle extends StatefulWidget {
  final Animation<double> enter;
  final ValueListenable<double>? amplitude;
  final bool video;
  final double slideDx;

  /// `slideToCancelProgress` (1 — joyida).
  final double slide;

  /// Qulf tomon surilgan ulush (0..1).
  final double lockMove;

  const _RecordCircle({
    required this.enter,
    required this.amplitude,
    required this.video,
    required this.slideDx,
    required this.slide,
    required this.lockMove,
  });

  @override
  State<_RecordCircle> createState() => _RecordCircleState();
}

class _RecordCircleState extends State<_RecordCircle>
    with SingleTickerProviderStateMixin {
  late final Ticker _ticker;
  final _big = _Blob(12);
  final _tiny = _Blob(11);
  final _repaint = ValueNotifier<int>(0);
  Duration _last = Duration.zero;

  /// Doira radiusidagi ovoz ulushi (`RecordCircle.amplitude`).
  double _amp = 0;
  double _ampTo = 0;
  double _ampStep = 0;

  /// Qulfning "nafas olishi" (`idleProgress`, 0..1..0).
  double _idle = 0;
  bool _idleUp = true;

  @override
  void initState() {
    super.initState();
    _big
      ..minRadius = 50
      ..maxRadius = 50 + 12 * _Blob.formBigMax;
    _tiny
      ..minRadius = 47
      ..maxRadius = 47 + 15 * _Blob.formSmallMax;
    _big.generate();
    _tiny.generate();
    widget.amplitude?.addListener(_onAmp);
    _ticker = createTicker(_tick)..start();
  }

  @override
  void didUpdateWidget(_RecordCircle old) {
    super.didUpdateWidget(old);
    if (old.amplitude != widget.amplitude) {
      old.amplitude?.removeListener(_onAmp);
      widget.amplitude?.addListener(_onAmp);
    }
  }

  void _onAmp() {
    final v = (widget.amplitude?.value ?? 0).clamp(0.0, 1.0);
    _big.setValue(v, true);
    _tiny.setValue(v, false);
    _ampTo = v;
    // `animateAmplitudeDiff` (`WaveDrawable.animationSpeedCircle` = 0.55).
    _ampStep = (_ampTo - _amp) / (100 + 500 * 0.55);
  }

  void _tick(Duration now) {
    final dt = (now - _last).inMilliseconds.clamp(0, 50).toDouble();
    _last = now;
    if (_amp != _ampTo) {
      _amp += _ampStep * dt;
      if ((_ampStep > 0 && _amp > _ampTo) ||
          (_ampStep < 0 && _amp < _ampTo)) {
        _amp = _ampTo;
      }
    }
    _big.updateAmplitude(dt);
    _big.update(_big.amplitude, 1.01);
    _tiny.updateAmplitude(dt);
    _tiny.update(_tiny.amplitude, 1.02);
    if (_idleUp) {
      _idle += 0.01;
      if (_idle > 1) {
        _idle = 1;
        _idleUp = false;
      }
    } else {
      _idle -= 0.01;
      if (_idle < 0) {
        _idle = 0;
        _idleUp = true;
      }
    }
    _repaint.value++;
  }

  @override
  void dispose() {
    widget.amplitude?.removeListener(_onAmp);
    _ticker.dispose();
    _repaint.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      painter: _RecordPainter(
        repaint: Listenable.merge([_repaint, widget.enter]),
        state: this,
      ),
      child: const SizedBox.expand(),
    );
  }
}

class _RecordPainter extends CustomPainter {
  final _RecordCircleState state;

  _RecordPainter({required Listenable repaint, required this.state})
      : super(repaint: repaint);

  @override
  void paint(Canvas canvas, Size size) {
    final w = state.widget;
    // Tugma markazi: 150 x 230 (`Positioned` ga qarang).
    final cx = 150.0;
    final cy = 230.0;
    final scale = Curves.linear.transform(w.enter.value);
    // `sc`: 0 -> 1 (0.5 gacha), 1 -> 0.9, 0.9 -> 1.
    final double sc;
    if (scale <= 0.5) {
      sc = scale / 0.5;
    } else if (scale <= 0.75) {
      sc = 1 - (scale - 0.5) / 0.25 * 0.1;
    } else {
      sc = 0.9 + (scale - 0.75) / 0.25 * 0.1;
    }
    final slideScale = 0.7 + w.slide * 0.3;
    final radius = (41 + 30 * state._amp) * sc * slideScale;
    final x = cx + w.slideDx;
    const color = AppColors.accent;

    // ── Pufak to'lqinlar ─────────────────────────────────────
    final slide1 = w.slide > 0.7 ? 1.0 : w.slide / 0.7;
    final enter = Curves.easeOut.transform(sc.clamp(0.0, 1.0));
    if (slide1 > 0) {
      var s = sc * slide1 * enter * (_Blob.scaleBigMin + 1.4 * state._big.amplitude);
      canvas.save();
      canvas.translate(x, cy);
      canvas.scale(s, s);
      state._big.draw(canvas, Paint()..color = color.withValues(alpha: 0.30));
      canvas.restore();
      s = sc * slide1 * enter * (_Blob.scaleSmallMin + 1.4 * state._tiny.amplitude);
      canvas.save();
      canvas.translate(x, cy);
      canvas.scale(s, s);
      state._tiny.draw(canvas, Paint()..color = color.withValues(alpha: 0.15));
      canvas.restore();
    }

    // ── Doira va belgi ───────────────────────────────────────
    canvas.drawCircle(Offset(x, cy), radius, Paint()..color = color);
    final icon = w.video ? Icons.videocam_rounded : Icons.mic_rounded;
    final tp = TextPainter(
      text: TextSpan(
        text: String.fromCharCode(icon.codePoint),
        style: TextStyle(
          fontFamily: icon.fontFamily,
          package: icon.fontPackage,
          fontSize: 26 * sc.clamp(0.0, 1.0),
          color: Colors.white,
        ),
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    tp.paint(canvas, Offset(x - tp.width / 2, cy - tp.height / 2));

    // ── Qulf (`ControlsView`) ────────────────────────────────
    final move = 1 - w.lockMove.clamp(0.0, 1.0); // `moveProgress`
    final yAdd = w.lockMove.clamp(0.0, 1.0) * 57;
    final lockH = 36 + 14 * move;
    final lockTop =
        cy - 170 + 60 + 30 * (1 - sc) - yAdd + move * state._idle * -8;
    final alpha = (w.slide.clamp(0.0, 1.0) * sc.clamp(0.0, 1.0));
    if (alpha > 0.01) {
      final r = RRect.fromRectAndRadius(
          Rect.fromLTWH(cx - 18, lockTop, 36, lockH), const Radius.circular(18));
      canvas.drawRRect(
          r, Paint()..color = const Color(0xFF26292E).withValues(alpha: alpha));
      canvas.drawRRect(
          r,
          Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = 1
            ..color = Colors.white.withValues(alpha: 0.10 * alpha));
      final ic = Paint()
        ..color = Colors.white.withValues(alpha: 0.85 * alpha)
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.7
        ..strokeCap = StrokeCap.round;
      // Qulf tanasi va halqasi (`lockMiddleY`, `lockTopY`).
      final mid = lockTop + lockH / 2 - 8 + 2 + 2 * move;
      final body = RRect.fromRectAndRadius(
          Rect.fromCenter(center: Offset(cx, mid + 3), width: 13, height: 10),
          const Radius.circular(2.5));
      canvas.drawRRect(body, Paint()..color = Colors.white.withValues(alpha: 0.85 * alpha));
      final top = mid - 2;
      final shackle = Path()
        ..moveTo(cx - 4, top)
        ..lineTo(cx - 4, top - 3)
        ..arcToPoint(Offset(cx + 4, top - 3),
            radius: const Radius.circular(4))
        // Surilgan sari halqa yopiladi (`lockRotation`).
        ..lineTo(cx + 4, top - 3 + 3 * (1 - move));
      canvas.save();
      canvas.translate(cx, top);
      canvas.rotate(9 * move * math.pi / 180);
      canvas.translate(-cx, -top);
      canvas.drawPath(shackle, ic);
      canvas.restore();
      // Tepaga o'q.
      if (move > 0.05) {
        final ay = lockTop + lockH - 10;
        final arrow = Path()
          ..moveTo(cx - 4, ay + 2)
          ..lineTo(cx, ay - 2)
          ..lineTo(cx + 4, ay + 2);
        canvas.drawPath(
            arrow,
            ic
              ..color = Colors.white.withValues(alpha: 0.6 * alpha * move));
      }
    }
  }

  @override
  bool shouldRepaint(_RecordPainter old) => true;
}

/// `BlobDrawable` — tasodifiy nuqtali, silliq egri "pufak".
class _Blob {
  static const maxSpeed = 8.2;
  static const minSpeed = 0.8;
  static const scaleBigMin = 0.878;
  static const scaleSmallMin = 0.926;
  static const formBigMax = 0.6;
  static const formSmallMax = 0.6;

  final int n;
  final double _l;
  final _rnd = math.Random();
  late final List<double> _r = List.filled(n, 0);
  late final List<double> _a = List.filled(n, 0);
  late final List<double> _rNext = List.filled(n, 0);
  late final List<double> _aNext = List.filled(n, 0);
  late final List<double> _p = List.filled(n, 0);
  late final List<double> _speed = List.filled(n, 0);
  double minRadius = 0;
  double maxRadius = 0;
  double amplitude = 0;
  double _to = 0;
  double _diff = 0;

  _Blob(this.n) : _l = (4.0 / 3.0) * math.tan(math.pi / (2 * n));

  double _r100() => (_rnd.nextInt(200) - 100) / 100;

  void _gen(List<double> r, List<double> a, int i) {
    final angleDif = 360 / n * 0.05;
    final radDif = maxRadius - minRadius;
    r[i] = minRadius + _r100().abs() * radDif;
    a[i] = 360 / n * i + _r100() * angleDif;
    _speed[i] = 0.017 + 0.003 * _r100().abs();
  }

  void generate() {
    for (var i = 0; i < n; i++) {
      _gen(_r, _a, i);
      _gen(_rNext, _aNext, i);
      _p[i] = 0;
    }
  }

  void update(double amp, double speedScale) {
    for (var i = 0; i < n; i++) {
      _p[i] += _speed[i] * minSpeed + amp * _speed[i] * maxSpeed * speedScale;
      if (_p[i] >= 1) {
        _p[i] = 0;
        _r[i] = _rNext[i];
        _a[i] = _aNext[i];
        _gen(_rNext, _aNext, i);
      }
    }
  }

  void setValue(double v, bool big) {
    _to = v;
    // `ANIMATION_SPEED_WAVE_HUGE` 0.65, `_SMALL` 0.45.
    final speed = big ? 1 - 0.65 : 1 - 0.45;
    if (_to > amplitude) {
      _diff = (_to - amplitude) / (100 + (big ? 300 : 400) * speed);
    } else {
      _diff = (_to - amplitude) / (100 + 500 * speed);
    }
  }

  void updateAmplitude(double dt) {
    if (_to == amplitude) return;
    amplitude += _diff * dt;
    if ((_diff > 0 && amplitude > _to) || (_diff < 0 && amplitude < _to)) {
      amplitude = _to;
    }
  }

  /// Markaz (0, 0) atrofida chizadi.
  void draw(Canvas canvas, Paint paint) {
    final path = Path();
    for (var i = 0; i < n; i++) {
      final p = _p[i];
      final j = i + 1 < n ? i + 1 : 0;
      final pn = _p[j];
      final r1 = _r[i] * (1 - p) + _rNext[i] * p;
      final r2 = _r[j] * (1 - pn) + _rNext[j] * pn;
      final a1 = (_a[i] * (1 - p) + _aNext[i] * p) * math.pi / 180;
      final a2 = (_a[j] * (1 - pn) + _aNext[j] * pn) * math.pi / 180;
      final l = _l * (math.min(r1, r2) + (math.max(r1, r2) - math.min(r1, r2)) / 2);
      Offset rot(double x, double y, double a) => Offset(
          x * math.cos(a) - y * math.sin(a), x * math.sin(a) + y * math.cos(a));
      final s0 = rot(0, -r1, a1);
      final s1 = rot(l, -r1, a1);
      final e0 = rot(0, -r2, a2);
      final e1 = rot(-l, -r2, a2);
      if (i == 0) path.moveTo(s0.dx, s0.dy);
      path.cubicTo(s1.dx, s1.dy, e1.dx, e1.dy, e0.dx, e0.dy);
    }
    canvas.drawPath(path, paint);
  }
}
