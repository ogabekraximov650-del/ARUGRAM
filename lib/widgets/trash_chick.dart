// lib/widgets/trash_chick.dart — "KESH TOZALANMOQDA" ANIMATSIYASI.
//
// TALAB (foydalanuvchi): supurayotgan jo'ja o'rniga — jo'ja chap tomondagi
// eshikni ochib chiqadi, qo'lidagi axlat qopini axlat qutisiga tashlaydi
// va ortiga qaytib kiradi (takrorlanadi).
//
// Hammasi kod bilan chiziladi (`CustomPainter`, tashqi fayl yo'q). Sahna
// 260 x 170 mantiqiy birlikda yozilgan va berilgan o'lchamga sig'diriladi.
// Bitta aylanish 4.2 s:
//   0.00-0.08  eshik ochiladi
//   0.07-0.40  jo'ja qop bilan quti tomon yuradi
//   0.40-0.48  quti qopqog'i ochiladi
//   0.47-0.60  qo'l ko'tariladi, qop yoy bo'ylab qutiga uchadi
//   0.60-0.68  qopqoq sakrab yopiladi, chang ko'tariladi
//   0.66-0.74  jo'ja xursand sakraydi (ko'zlari yumuq kulgi)
//   0.72-0.93  orqaga o'girilib eshikka qaytadi
//   0.90-0.99  eshik yopiladi

import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

class TrashChickAnimation extends StatefulWidget {
  final double width;
  final double height;
  const TrashChickAnimation({super.key, this.width = 260, this.height = 170});

  @override
  State<TrashChickAnimation> createState() => _TrashChickAnimationState();
}

class _TrashChickAnimationState extends State<TrashChickAnimation>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 4200))
    ..repeat();

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return RepaintBoundary(
      child: CustomPaint(
        size: Size(widget.width, widget.height),
        painter: _ScenePainter(_c),
      ),
    );
  }
}

// ── SAHNA O'LCHAMLARI ────────────────────────────────────────────
const double _w = 260, _h = 170, _floor = 152;
const double _doorL = 14, _doorR = 70, _doorT = 58;
const double _binL = 198, _binR = 238, _binT = 112;
const double _home = 38, _stop = 150;

double _clamp(double v, double a, double b) => math.max(a, math.min(b, v));
double _seg(double t, double a, double b) => _clamp((t - a) / (b - a), 0, 1);
double _eio(double x) =>
    x < .5 ? 4 * x * x * x : 1 - math.pow(-2 * x + 2, 3).toDouble() / 2;
double _eout(double x) => 1 - math.pow(1 - x, 3).toDouble();
double _backOut(double x) {
  const c1 = 1.70158, c3 = c1 + 1;
  return 1 + c3 * math.pow(x - 1, 3).toDouble() + c1 * math.pow(x - 1, 2).toDouble();
}

double _lerp(double a, double b, double t) => a + (b - a) * t;

class _State {
  final double t, door, x, face, lid, thr, hop, puff;
  final bool walking;
  _State(this.t)
      : door = _seg(t, 0, .08) - _seg(t, .9, .99),
        x = _lerp(_lerp(_home, _stop, _eio(_seg(t, .07, .40))), _home,
            _eio(_seg(t, .74, .93))),
        walking = (t > .07 && t < .40) || (t > .74 && t < .93),
        face = t < .72
            ? 1.0
            : (t < .76 ? _lerp(1, -1, _seg(t, .72, .76)) : -1.0),
        lid = _eout(_seg(t, .40, .48)) * (1 - _backOut(_seg(t, .60, .68))),
        thr = _seg(t, .47, .60),
        hop = t > .66 && t < .74 ? math.sin(_seg(t, .66, .74) * math.pi) : 0.0,
        puff = _seg(t, .62, .78);

  double get bob => walking ? (math.sin(t * 60)).abs() * 2.5 : 0;

  /// Qo'lning ko'tarilishi (otish boshida).
  double get raise => thr > 0 && thr < 1
      ? math.sin(math.min(thr / .35, 1) * math.pi / 2)
      : 0.0;
}

class _ScenePainter extends CustomPainter {
  final Animation<double> anim;
  _ScenePainter(this.anim) : super(repaint: anim);

  @override
  void paint(Canvas canvas, Size size) {
    final k = math.min(size.width / _w, size.height / _h);
    canvas.save();
    canvas.translate((size.width - _w * k) / 2, (size.height - _h * k) / 2);
    canvas.scale(k);
    final s = _State(anim.value);

    // pol soyasi
    canvas.drawOval(
        Rect.fromCenter(center: const Offset(130, _floor + 2), width: 240, height: 10),
        Paint()..color = const Color(0x0FFFFFFF));
    _doorBack(canvas, s);
    _frame(canvas);
    // Jo'ja eshik ichidan chiqadi: ramkaning chap chetidan tashqarisi ko'rinmaydi.
    canvas.save();
    canvas.clipRect(const Rect.fromLTWH(_doorL + 4, 0, _w, _h));
    _chick(canvas, s);
    canvas.restore();
    _doorPanel(canvas, s);
    _bin(canvas, s);
    _bag(canvas, s);
    _puff(canvas, s);
    canvas.restore();
  }

  static final RRect _doorRect = RRect.fromLTRBR(
      _doorL, _doorT, _doorR, _floor, const Radius.circular(6));

  void _doorBack(Canvas canvas, _State s) {
    canvas.drawRRect(_doorRect, Paint()..color = const Color(0xFF231A15));
    canvas.drawRRect(
        _doorRect,
        Paint()
          ..color = const Color(0xFFFFD68C).withValues(alpha: .18 * s.door));
  }

  void _frame(Canvas canvas) {
    final p = Path()
      ..moveTo(_doorL, _floor)
      ..lineTo(_doorL, _doorT + 6)
      ..quadraticBezierTo(_doorL, _doorT, _doorL + 6, _doorT)
      ..lineTo(_doorR - 6, _doorT)
      ..quadraticBezierTo(_doorR, _doorT, _doorR, _doorT + 6)
      ..lineTo(_doorR, _floor);
    canvas.drawPath(
        p,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 5
          ..strokeJoin = StrokeJoin.round
          ..color = const Color(0xFF6B4A36));
  }

  /// Eshik tabaqasi chap ilmoqda aylanadi (perspektiva: torayadi va qiyshayadi).
  void _doorPanel(Canvas canvas, _State s) {
    final o = s.door;
    final w = (_doorR - _doorL - 3) * (1 - .82 * o);
    final skew = 12 * o;
    const x0 = _doorL + 1.5;
    final x1 = x0 + w;
    final p = Path()
      ..moveTo(x0, _doorT + 2)
      ..lineTo(x1, _doorT + 2 - skew)
      ..lineTo(x1, _floor + skew * .6)
      ..lineTo(x0, _floor)
      ..close();
    final open = o > .5;
    canvas.drawPath(
        p,
        Paint()
          ..shader = ui.Gradient.linear(Offset(x0, 0), Offset(x1, 0), [
            open ? const Color(0xFF8A5A3C) : const Color(0xFFC7844F),
            open ? const Color(0xFFA86C45) : const Color(0xFFB87747),
          ]));
    canvas.drawPath(
        p,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2.5
          ..strokeJoin = StrokeJoin.round
          ..color = const Color(0xFF6B4A36));
    if (w > 14) {
      final line = Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.6
        ..color = const Color(0xB36B4A36);
      final px = x0 + w * .18, pw = w * .64;
      canvas.drawRect(Rect.fromLTWH(px, _doorT + 12 - skew * .3, pw, 26), line);
      canvas.drawRect(Rect.fromLTWH(px, _doorT + 46 - skew * .3, pw, 34), line);
      canvas.drawCircle(Offset(x0 + w * .84, _doorT + 52 - skew * .2), 2.6,
          Paint()..color = const Color(0xFFF2C94C));
    }
  }

  void _bin(Canvas canvas, _State s) {
    final body = Path()
      ..moveTo(_binL + 2, _binT)
      ..lineTo(_binR - 2, _binT)
      ..lineTo(_binR - 5, _floor)
      ..lineTo(_binL + 5, _floor)
      ..close();
    canvas.drawPath(body, Paint()..color = const Color(0xFF4F7D8C));
    final edge = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2.5
      ..strokeJoin = StrokeJoin.round
      ..color = const Color(0xFF355866);
    canvas.drawPath(body, edge);
    final rib = Paint()
      ..strokeWidth = 2
      ..color = const Color(0xCC355866);
    for (final f in const [.3, .5, .7]) {
      final x = _lerp(_binL + 5, _binR - 5, f);
      canvas.drawLine(Offset(x, _binT + 8), Offset(x, _floor - 7), rib);
    }
    // Qopqoq — chap chetida ilmoq.
    canvas.save();
    canvas.translate(_binL, _binT - 1);
    canvas.rotate(-1.9 * s.lid);
    final lid = RRect.fromLTRBR(-2, -6, _binR - _binL + 2, 1, const Radius.circular(3));
    canvas.drawRRect(lid, Paint()..color = const Color(0xFF5F93A3));
    canvas.drawRRect(lid, edge);
    const hw = (_binR - _binL) / 2;
    canvas.drawRRect(RRect.fromLTRBR(hw - 6, -10, hw + 6, -6, const Radius.circular(2)),
        Paint()..color = const Color(0xFF355866));
    canvas.restore();
  }

  /// Qop turadigan joy (jo'janing oldingi qo'li).
  Offset _hand(_State s, [double? thr]) {
    final y = _floor - 6 - s.bob - s.hop * 14;
    final t = thr ?? s.thr;
    final raise =
        t > 0 && t < 1 ? math.sin(math.min(t / .35, 1) * math.pi / 2) : 0.0;
    return Offset(s.x + s.face * 24, y - 18 - raise * 16);
  }

  void _bag(Canvas canvas, _State s) {
    if (s.thr >= 1 || s.t > .60) return; // qutida
    var p = _hand(s);
    var sc = 1.0, rot = 0.0;
    if (s.thr > .35) {
      // Yoy bo'ylab qutiga uchadi.
      final u = _eio((s.thr - .35) / .65);
      final a = _hand(s, .35);
      const bx = (_binL + _binR) / 2, by = _binT + 4;
      p = Offset(_lerp(a.dx, bx, u), _lerp(a.dy, by, u) - math.sin(u * math.pi) * 34);
      rot = u * math.pi * 1.6;
      sc = 1 - .3 * _seg(u, .8, 1);
    }
    canvas.save();
    canvas.translate(p.dx, p.dy);
    canvas.rotate(rot);
    canvas.scale(sc);
    final fill = Paint()..color = const Color(0xFF3C3F45);
    final edge = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeJoin = StrokeJoin.round
      ..color = const Color(0xFF23252A);
    final sack = Path()
      ..moveTo(-3, -8)
      ..cubicTo(-14, -4, -13, 10, 0, 10)
      ..cubicTo(13, 10, 14, -4, 3, -8)
      ..close();
    final knot = Path()
      ..moveTo(-3, -8)
      ..lineTo(-6, -14)
      ..lineTo(0, -11)
      ..lineTo(6, -14)
      ..lineTo(3, -8)
      ..close();
    for (final q in [sack, knot]) {
      canvas.drawPath(q, fill);
      canvas.drawPath(q, edge);
    }
    canvas.save();
    canvas.translate(-4, 0);
    canvas.rotate(-.4);
    canvas.drawOval(Rect.fromCenter(center: Offset.zero, width: 5, height: 8),
        Paint()..color = const Color(0x2EFFFFFF));
    canvas.restore();
    canvas.restore();
  }

  void _puff(Canvas canvas, _State s) {
    if (s.puff <= 0 || s.puff >= 1) return;
    final a = 1 - s.puff;
    const cx = (_binL + _binR) / 2;
    final paint = Paint()..color = Color.fromRGBO(200, 200, 200, .55 * a);
    for (final d in const [
      [-14.0, -4.0, 5.0],
      [-5.0, -9.0, 6.0],
      [6.0, -8.0, 5.0],
      [15.0, -3.0, 4.5],
    ]) {
      canvas.drawCircle(
          Offset(cx + d[0] * (1 + s.puff * .8), _binT - 6 + d[1] * (1 + s.puff)),
          d[2] * (.6 + s.puff * .6),
          paint);
    }
  }

  void _chick(Canvas canvas, _State s) {
    final f = s.face;
    final base = _floor - s.bob - s.hop * 14;
    final x = s.x;
    // soya
    canvas.drawOval(
        Rect.fromCenter(
            center: Offset(x, _floor + 1), width: 2 * (20 - 6 * s.hop), height: 8),
        Paint()..color = Color.fromRGBO(0, 0, 0, .25 - .12 * s.hop));
    // oyoqlar
    final st = s.walking ? math.sin(s.t * 60) : 0.0;
    final footFill = Paint()..color = const Color(0xFFF08A24);
    final footEdge = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5
      ..color = const Color(0xFFC46412);
    for (final k in const [-1.0, 1.0]) {
      final double lx = x + k * 8 + (s.walking ? k * st * 3 : 0.0);
      final double ly = base + (s.walking ? math.max(0.0, k * st) * -3 : 0.0);
      final r = Rect.fromCenter(center: Offset(lx + f * 2, ly - 2), width: 12, height: 6);
      canvas.drawOval(r, footFill);
      canvas.drawOval(r, footEdge);
    }

    canvas.save();
    canvas.translate(x, base - 4);
    // O'girilish: eniga siqilib, teskari tomonga ochiladi.
    canvas.scale((f < 0 ? -1.0 : 1.0) * math.max(f.abs(), .15), 1);
    final double sq = 1.0 + (s.walking ? .04 * math.sin(s.t * 120) : 0.0);
    canvas.scale(1 / sq, sq);

    final body = Path()
      ..moveTo(0, -62)
      ..cubicTo(20, -62, 28, -46, 26, -30)
      ..cubicTo(33, -16, 30, 0, 12, 0)
      ..lineTo(-12, 0)
      ..cubicTo(-30, 0, -33, -16, -26, -30)
      ..cubicTo(-28, -46, -20, -62, 0, -62)
      ..close();
    canvas.drawPath(
        body,
        Paint()
          ..shader = ui.Gradient.radial(const Offset(-4, -38), 40,
              const [Color(0xFFFFE066), Color(0xFFF2B822)]));
    final outline = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3
      ..strokeJoin = StrokeJoin.round
      ..color = const Color(0xFFE0801C);
    canvas.drawPath(body, outline);
    // yaltirash
    canvas.drawArc(
        Rect.fromCircle(center: const Offset(-4, -40), radius: 17),
        math.pi * 1.1,
        math.pi * .35,
        false,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2.5
          ..strokeCap = StrokeCap.round
          ..color = const Color(0xBFFFFFFF));

    // ko'zlar: xursand paytda yumuq kulgi, aks holda vaqti-vaqti bilan miltillaydi
    final happy = s.hop > 0 || (s.t > .60 && s.t < .74);
    final blink = (s.t % .5) > .47 ? .15 : 1.0;
    const ink = Color(0xFF1D1D1D);
    for (final ex in const [3.0, 17.0]) {
      if (happy) {
        canvas.drawArc(
            Rect.fromCircle(center: Offset(ex, -40), radius: 3.5),
            math.pi * 1.1,
            math.pi * .8,
            false,
            Paint()
              ..style = PaintingStyle.stroke
              ..strokeWidth = 2.2
              ..strokeCap = StrokeCap.round
              ..color = ink);
      } else {
        canvas.drawOval(
            Rect.fromCenter(center: Offset(ex, -41), width: 5.2, height: 7.2 * blink),
            Paint()..color = ink);
        if (blink > .5) {
          canvas.drawCircle(
              Offset(ex + .8, -42.5), .9, Paint()..color = Colors.white);
        }
      }
    }
    // yonoqlar
    final cheek = Paint()..color = const Color(0x73FF785A);
    canvas.drawOval(Rect.fromCenter(center: const Offset(-1, -33), width: 7, height: 4.4), cheek);
    canvas.drawOval(Rect.fromCenter(center: const Offset(22, -33), width: 6, height: 4), cheek);
    // tumshuq
    final beak = Path()
      ..moveTo(8, -37)
      ..quadraticBezierTo(19, -35, 21, -32)
      ..quadraticBezierTo(15, -29, 8, -31)
      ..close();
    canvas.drawPath(beak, Paint()..color = const Color(0xFFF37B2B));
    canvas.drawPath(
        beak,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.5
          ..strokeJoin = StrokeJoin.round
          ..color = const Color(0xFFC4551A));

    // Oldingi qo'l: qopni ushlaydi, otishda ko'tariladi, sakraganda silkinadi.
    final raise = s.raise * (s.thr < .5 ? 1 : 1 - _seg(s.thr, .5, .8));
    final wave = s.t > .66 && s.t < .74
        ? math.sin(_seg(s.t, .66, .74) * math.pi * 4) * .4
        : 0.0;
    final wingEdge = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2.5
      ..color = const Color(0xFFE0801C);
    canvas.save();
    canvas.translate(12, -20);
    canvas.rotate(-.2 - raise * 1.1 + wave);
    final front = Rect.fromCenter(center: const Offset(8, 0), width: 20, height: 11);
    canvas.drawOval(front, Paint()..color = const Color(0xFFF5C330));
    canvas.drawOval(front, wingEdge);
    canvas.restore();
    // orqa qanot
    canvas.save();
    canvas.translate(-22, -22);
    canvas.rotate(.4);
    final back = Rect.fromCenter(center: Offset.zero, width: 10, height: 18);
    canvas.drawOval(back, Paint()..color = const Color(0xFFF0B21F));
    canvas.drawOval(back, wingEdge);
    canvas.restore();

    canvas.restore();
  }

  @override
  bool shouldRepaint(_ScenePainter old) => old.anim != anim;
}
