// lib/widgets/trash_chick.dart — "KESH TOZALANMOQDA" ANIMATSIYASI.
//
// TALAB (foydalanuvchi): Telegram'dagi o'sha jo'ja (`utyan`) chap tomondagi
// eshikni ochib chiqadi, qo'lidagi axlat qopini axlat qutisiga tashlaydi va
// ortiga qaytib kiradi; burilganda qog'oz parchasidek yassilanmasdan, HAQIQIY
// tanasi bilan aylansin; professional sifatda.
//
// ── QANDAY QILINGAN ─────────────────────────────────────────────
//
// * Jo'ja — `utyan_cache.json` (Lottie) dagi asl vektor shakllar
//   (`utyan_parts.dart`): tana, bosh, tumshuq, og'iz, ko'zlar, qo'llar, yaltirashlar
//   — o'sha rang va chiziq qalinligida.
// * Burilish: Lottie'ning o'zida jo'ja boshini o'ngga buradi (kadr 106 -> 200).
//   Ikki holatning nuqtalari bir-biriga mos, shuning uchun ular nuqtama-nuqta
//   aralashtiriladi — ko'zlar, tumshuq bosh sirti bo'ylab suriladi, uzoqdagi ko'z
//   chetga kirib torayadi, tana esa o'z shaklida qoladi. Chapga burilish — o'ngga
//   burilgan holatning ko'zgudagi aksi tomon siljish (yuz qismlari o'rin
//   almashadi), qo'llar tana chetidan chiqmaydi.
// * Harakat: lapanglab sakrab yurish (qo'nishda yassilanadi), otishdan oldin
//   cho'kib orqaga og'ish, otishda cho'zilish, xursandlikda ko'zlar "^^" bo'lib
//   sakrash. Eshik 3D perspektivada ochiladi, ichkaridan iliq yorug'lik tushadi;
//   qutining qopqog'i ochilib, sakrab yopiladi, chang va uchqunlar chiqadi.
//
// Sahna 1040 x 680 birlikda yozilgan va berilgan o'lchamga sig'diriladi.
// Bitta aylanish 5.4 s, takrorlanadi.

import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import 'utyan_parts.dart';

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
      vsync: this, duration: const Duration(milliseconds: 5400))
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

// ── SAHNA O'LCHAMLARI ─────────────────────────────────────────────
const double _w = 1040, _h = 680, _floor = 600;
const double _dl = 40, _dr = 300, _dt = 190; // eshik
const double _bl = 790, _br = 960, _bt = 410; // quti
const double _home = 170, _stop = 560; // jo'ja yo'li
const double _k = 0.64; // jo'ja masshtabi (512 -> sahna)
const double _px = 294, _py = 392; // jo'janing tayanch nuqtasi (512)
const double _headC = 302; // boshning o'qi (chapga burilish ko'zgusi)
const double _out = 9; // sahna chiziqlari qalinligi

const Color _wood = Color(0xFFAA5A1F);
const Color _woodInk = Color(0xFF6D3605);
const Color _binInk = Color(0xFF1B4D59);
const Color _bagInk = Color(0xFF14161A);
const Color _bagFill = Color(0xFF3B3F47);
const Color _dust = Color(0xFFC7CCC1);
const Color _gold = Color(0xFFFFD527);

// ── YUMSHATISH FUNKSIYALARI ──────────────────────────────────────
double _clamp(double v, double a, double b) => math.max(a, math.min(b, v));
double _seg(double t, double a, double b) => _clamp((t - a) / (b - a), 0, 1);
double _lerp(double a, double b, double t) => a + (b - a) * t;
double _eio(double x) =>
    x < .5 ? 4 * x * x * x : 1 - math.pow(-2 * x + 2, 3).toDouble() / 2;
double _eout(double x) => 1 - math.pow(1 - x, 3).toDouble();
double _sio(double x) => -(math.cos(math.pi * x) - 1) / 2;
double _backOut(double x, [double s = 1.7]) =>
    1 + (s + 1) * math.pow(x - 1, 3).toDouble() + s * math.pow(x - 1, 2).toDouble();
double _bounce(double x) {
  const n = 7.5625, d = 2.75;
  if (x < 1 / d) return n * x * x;
  if (x < 2 / d) {
    final y = x - 1.5 / d;
    return n * y * y + .75;
  }
  if (x < 2.5 / d) {
    final y = x - 2.25 / d;
    return n * y * y + .9375;
  }
  final y = x - 2.625 / d;
  return n * y * y + .984375;
}

// ── VAQT JADVALI ─────────────────────────────────────────────────
class _S {
  final double t;
  late final double door, x, stepPh, yaw, lid, wind, fling, fly, puff, hop, spark;
  late final bool walking, hasBag, happy;

  _S(this.t) {
    door = _eio(_seg(t, 0, .10)) - _eio(_seg(t, .93, 1));
    final go = _seg(t, .12, .44), back = _seg(t, .80, .95);
    final out = go > 0 && go < 1, ret = back > 0 && back < 1;
    walking = out || ret;
    x = _lerp(_lerp(_home, _stop, _sio(go)), _home, _sio(back));
    stepPh = out ? go * 6 : (ret ? back * 6 : 0); // 6 qadam
    // Burilish: 0 — oldga, 1 — o'ngga, -1 — chapga.
    var y = _lerp(0, 1, _eio(_seg(t, .08, .14)));
    y = _lerp(y, 0, _eio(_seg(t, .66, .71)));
    y = _lerp(y, -1, _eio(_seg(t, .76, .81)));
    y = _lerp(y, 0, _eio(_seg(t, .93, .97)));
    yaw = y;
    lid = _backOut(_seg(t, .46, .53), 1.2) * (1 - _bounce(_seg(t, .615, .68)));
    // Otish: tayyorlanish (cho'kadi, orqaga og'adi), keyin siltanish.
    wind = _sio(_seg(t, .50, .545)) * (1 - _seg(t, .545, .57));
    fling = _eout(_seg(t, .545, .575)) * (1 - _sio(_seg(t, .60, .66)));
    fly = _seg(t, .565, .625);
    hasBag = t < .565;
    puff = _seg(t, .615, .74);
    happy = t > .66 && t < .79;
    hop = t > .69 && t < .77 ? math.sin(_seg(t, .69, .77) * math.pi) : 0.0;
    spark = _seg(t, .63, .80);
  }

  double get stepLift =>
      walking ? math.sin((stepPh % 1) * math.pi) * 22 : 0.0;
}

// ── 2D AFFIN (qop qo'ldan chiqadigan nuqtani hisoblash uchun) ─────
class _Aff {
  double a = 1, b = 0, c = 0, d = 1, e = 0, f = 0;
  void translate(Canvas? cv, double x, double y) {
    cv?.translate(x, y);
    e += a * x + c * y;
    f += b * x + d * y;
  }

  void scale(Canvas? cv, double sx, double sy) {
    cv?.scale(sx, sy);
    a *= sx;
    b *= sx;
    c *= sy;
    d *= sy;
  }

  void rotate(Canvas? cv, double r) {
    cv?.rotate(r);
    final co = math.cos(r), si = math.sin(r);
    final na = a * co + c * si, nb = b * co + d * si;
    final nc = -a * si + c * co, nd = -b * si + d * co;
    a = na;
    b = nb;
    c = nc;
    d = nd;
  }

  Offset map(double x, double y) => Offset(a * x + c * y + e, b * x + d * y + f);
}

// ── JO'JA SHAKLLARI ──────────────────────────────────────────────
int _argc(double op) => op == 1 ? 6 : (op == 3 ? 0 : 2);

List<double> _mix(List<double> a, List<double> b, double t) {
  final o = List<double>.of(a);
  var i = 0;
  while (i < a.length) {
    final n = _argc(a[i++]);
    for (var k = 0; k < n; k++, i++) {
      o[i] = a[i] + (b[i] - a[i]) * t;
    }
  }
  return o;
}

Rect _bbox(List<double> a) {
  var x0 = 1e9, y0 = 1e9, x1 = -1e9, y1 = -1e9;
  var i = 0;
  while (i < a.length) {
    final n = _argc(a[i++]);
    for (var k = 0; k < n; k += 2) {
      x0 = math.min(x0, a[i + k]);
      x1 = math.max(x1, a[i + k]);
      y0 = math.min(y0, a[i + k + 1]);
      y1 = math.max(y1, a[i + k + 1]);
    }
    i += n;
  }
  return Rect.fromLTRB(x0, y0, x1, y1);
}

/// Markaz atrofida eniga cho'zish va surish.
List<double> _xform(
    List<double> a, double cx, double sx, double dx, double dy) {
  final o = List<double>.of(a);
  var i = 0;
  while (i < a.length) {
    final n = _argc(a[i++]);
    for (var k = 0; k < n; k += 2) {
      o[i + k] = cx + (a[i + k] - cx) * sx + dx;
      o[i + k + 1] = a[i + k + 1] + dy;
    }
    i += n;
  }
  return o;
}

/// Ko'zni tik ag'daradi: yumuq "U" -> xursand "^".
List<double> _flipY(List<double> a) {
  final cy = _bbox(a).center.dy;
  final o = List<double>.of(a);
  var i = 0;
  while (i < a.length) {
    final n = _argc(a[i++]);
    for (var k = 0; k < n; k += 2) {
      o[i + k + 1] = 2 * cy - a[i + k + 1] + 6;
    }
    i += n;
  }
  return o;
}

Path _path(List<double> a) {
  final p = Path();
  var i = 0;
  while (i < a.length) {
    final op = a[i++];
    if (op == 0) {
      p.moveTo(a[i], a[i + 1]);
      i += 2;
    } else if (op == 1) {
      p.cubicTo(a[i], a[i + 1], a[i + 2], a[i + 3], a[i + 4], a[i + 5]);
      i += 6;
    } else if (op == 2) {
      p.lineTo(a[i], a[i + 1]);
      i += 2;
    } else {
      p.close();
    }
  }
  return p;
}

final Map<String, UtyanPart> _parts = {for (final p in kUtyanParts) p.name: p};

/// Chapga burilganda yuz qismlari o'rin almashadi (uzoqdagi ko'z — chetda).
const Map<String, String> _faceSwap = {
  'eye': 'eye_2',
  'eye_2': 'eye',
  'beak': 'beak',
  'beak_bl': 'beak_bl',
  'mouth': 'mouth',
};
const Set<String> _hands = {'hand1', 'hand2', 'hand_bl', 'hand__bl'};

List<double> _pose(String name, double yaw) {
  final part = _parts[name]!;
  if (yaw >= 0) return _mix(part.a, part.b, yaw);
  final u = -yaw;
  final ra = _bbox(part.a);
  double tx, ty, tw;
  final swap = _faceSwap[name];
  if (swap != null) {
    final rb = _bbox(_parts[swap]!.b);
    tx = 2 * _headC - rb.center.dx;
    ty = rb.center.dy;
    tw = rb.width;
  } else {
    final rb = _bbox(part.b);
    final k = _hands.contains(name) ? .2 : 1.0;
    tx = ra.center.dx - (rb.center.dx - ra.center.dx) * k;
    ty = rb.center.dy;
    tw = rb.width;
  }
  return _xform(part.a, ra.center.dx, _lerp(1, tw / ra.width, u),
      (tx - ra.center.dx) * u, (ty - ra.center.dy) * u);
}

class _ScenePainter extends CustomPainter {
  final Animation<double> anim;
  _ScenePainter(this.anim) : super(repaint: anim);

  /// Qop qo'ldan uziladigan nuqta (sahnada) — bir marta hisoblanadi.
  static Offset? _release;

  @override
  void paint(Canvas canvas, Size size) {
    final k = math.min(size.width / _w, size.height / _h);
    canvas.save();
    canvas.translate((size.width - _w * k) / 2, (size.height - _h * k) / 2);
    canvas.scale(k);
    final s = _S(anim.value);
    _release ??= _handWorld(_S(.565));

    // pol
    canvas.drawOval(
        Rect.fromCenter(
            center: const Offset(_w / 2, _floor + 10), width: _w * .94, height: 52),
        Paint()..color = const Color(0x0DFFFFFF));
    _doorBack(canvas, s);
    _binShadow(canvas);
    canvas.save();
    canvas.clipRect(const Rect.fromLTWH(_dl + 13, 0, _w, _h));
    _chick(canvas, s);
    canvas.restore();
    _doorPanel(canvas, s);
    _bin(canvas, s);
    _flyingBag(canvas, s);
    _puff(canvas, s);
    _sparks(canvas, s);
    canvas.restore();
  }

  // ── JO'JA ──────────────────────────────────────────────────────

  /// Tana o'zgarishi (yurish, otish, sakrash). [cv] null — faqat hisob.
  static _Aff _body(Canvas? cv, _S s) {
    var rot = 0.0, lift = s.stepLift, sx = 1.0, sy = 1.0;
    if (s.walking) {
      final ph = s.stepPh % 1, n = s.stepPh.floor();
      // lapanglash: har qadamda boshqa tomonga og'adi
      rot = (n.isOdd ? 1 : -1) * math.sin(ph * math.pi) * .07;
      final land = ph < .15 ? 1 - ph / .15 : (ph > .85 ? (ph - .85) / .15 : 0.0);
      sy = 1 - .07 * land;
      sx = 1 + .06 * land;
    }
    sy *= 1 - .08 * s.wind + .06 * s.fling;
    sx *= 1 + .07 * s.wind - .04 * s.fling;
    final dir = s.yaw < 0 ? -1.0 : 1.0;
    rot += (-.10 * s.wind + .12 * s.fling) * dir;
    lift += s.hop * 70;
    if (s.hop > 0) {
      sy *= 1 + .06 * s.hop;
      sx *= 1 - .04 * s.hop;
    }
    final m = _Aff();
    m.translate(cv, s.x, _floor - lift);
    m.rotate(cv, rot);
    m.scale(cv, _k * sx, _k * sy);
    m.translate(cv, -_px, -_py);
    return m;
  }

  static Offset _handsCenter(double yaw) {
    final r = _bbox(_pose('hand1', yaw)).expandToInclude(_bbox(_pose('hand2', yaw)));
    return r.center;
  }

  static double _armAngle(_S s) =>
      (-1.25 * s.fling + .5 * s.wind) * (s.yaw < 0 ? -1 : 1);

  static Offset _handWorld(_S s) {
    final m = _body(null, s);
    final h = _handsCenter(s.yaw);
    m.translate(null, h.dx, h.dy - 20);
    m.rotate(null, _armAngle(s));
    m.translate(null, -h.dx, -(h.dy - 20));
    return m.map(h.dx + 8, h.dy + 58);
  }

  void _part(Canvas canvas, String name, List<double> flat) {
    final p = _parts[name]!;
    final path = _path(flat);
    if (p.fill != null) canvas.drawPath(path, Paint()..color = p.fill!);
    if (p.stroke != null) {
      canvas.drawPath(
          path,
          Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = p.width
            ..strokeCap = p.butt ? StrokeCap.butt : StrokeCap.round
            ..strokeJoin = StrokeJoin.round
            ..color = p.stroke!);
    }
  }

  void _chick(Canvas canvas, _S s) {
    // soya: sakraganda kichrayib xiralashadi
    final lift = s.hop * 70 + s.stepLift;
    canvas.drawOval(
        Rect.fromCenter(
            center: Offset(s.x, _floor + 4),
            width: 220 * (1 - lift / 300),
            height: 32),
        Paint()..color = Color.fromRGBO(0, 0, 0, _clamp(.28 - lift / 400, 0, 1)));

    canvas.save();
    _body(canvas, s);
    final y = s.yaw;
    for (final n in const [
      'body', 'head_bl3', 'head', 'head_bl1', 'head_bl2', 'beak', 'beak_bl', 'mouth'
    ]) {
      _part(canvas, n, _pose(n, y));
    }
    for (final n in const ['eye', 'eye_2']) {
      final f = _pose(n, y);
      _part(canvas, n, s.happy ? _flipY(f) : f);
    }

    // qo'llar va qop
    final h = _handsCenter(y);
    canvas.save();
    canvas.translate(h.dx, h.dy - 20);
    canvas.rotate(_armAngle(s));
    canvas.translate(-h.dx, -(h.dy - 20));
    if (s.hasBag) _bag(canvas, h.dx + (y >= 0 ? 8 : -8), h.dy + 58, 0, .85, .85);
    if (s.happy && s.hop > 0) {
      canvas.translate(h.dx, h.dy);
      canvas.rotate(-.25 * math.sin(s.t * 90) * s.hop);
      canvas.translate(-h.dx, -h.dy);
    }
    // Chapga qaraganda qo'llar tana chetidan chiqmasin (tanani o'rab turgandek).
    if (y < 0) {
      canvas.clipPath(Path()
        ..addPath(_path(_pose('body', y)), Offset.zero)
        ..addPath(_path(_pose('head', y)), Offset.zero));
    }
    for (final n in const ['hand2', 'hand__bl', 'hand1', 'hand_bl']) {
      _part(canvas, n, _pose(n, y));
    }
    canvas.restore();
    canvas.restore();
  }

  // ── QOP ────────────────────────────────────────────────────────

  void _bag(Canvas canvas, double x, double y, double rot, double sx, double sy) {
    canvas.save();
    canvas.translate(x, y);
    canvas.rotate(rot);
    canvas.scale(sx, sy);
    final fill = Paint()..color = _bagFill;
    final ink = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 10
      ..strokeJoin = StrokeJoin.round
      ..color = _bagInk;
    final body = Path()
      ..moveTo(-18, -58)
      ..cubicTo(-70, -40, -78, 40, -40, 62)
      ..cubicTo(-15, 76, 15, 76, 40, 62)
      ..cubicTo(78, 40, 70, -40, 18, -58)
      ..close();
    canvas.drawPath(body, fill);
    canvas.drawPath(body, ink);
    final knot = Path()
      ..moveTo(-18, -58)
      ..cubicTo(-40, -78, -48, -96, -30, -100)
      ..cubicTo(-18, -100, -8, -80, 0, -70)
      ..cubicTo(8, -80, 18, -100, 30, -100)
      ..cubicTo(48, -96, 40, -78, 18, -58)
      ..close();
    canvas.drawPath(knot, fill);
    canvas.drawPath(knot, ink);
    canvas.drawOval(Rect.fromCenter(center: const Offset(0, -62), width: 32, height: 18),
        Paint()..color = _bagInk);
    final fold = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 6
      ..strokeCap = StrokeCap.round
      ..color = const Color(0xCC14161A);
    canvas.drawPath(
        Path()
          ..moveTo(-6, -40)
          ..quadraticBezierTo(-14, -10, -8, 20),
        fold);
    canvas.drawPath(
        Path()
          ..moveTo(22, -36)
          ..quadraticBezierTo(30, 0, 20, 30),
        fold);
    canvas.drawPath(
        Path()
          ..moveTo(-44, -10)
          ..quadraticBezierTo(-48, 15, -36, 36),
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 8
          ..strokeCap = StrokeCap.round
          ..color = const Color(0x8CFFFFFF));
    canvas.restore();
  }

  void _flyingBag(Canvas canvas, _S s) {
    if (s.t < .565 || s.fly >= 1) return;
    final u = s.fly;
    final a = _release!;
    const bx = (_bl + _br) / 2, by = _bt - 10;
    final x = _lerp(a.dx, bx, u);
    final y = _lerp(a.dy, by, u) -
        math.sin(u * math.pi) * 170 +
        (u > .8 ? (u - .8) * 300 : 0);
    final st = 1 + .12 * math.sin(u * math.pi);
    final shrink = 1 - .35 * _seg(u, .85, 1);
    _bag(canvas, x, y, u * math.pi * 1.3, .85 * _k / st * shrink,
        .85 * _k * st * shrink);
  }

  // ── ESHIK ──────────────────────────────────────────────────────

  void _doorBack(Canvas canvas, _S s) {
    final r = RRect.fromLTRBR(_dl, _dt, _dr, _floor, const Radius.circular(22));
    canvas.drawRRect(
        r,
        Paint()
          ..shader = ui.Gradient.linear(const Offset(0, _dt), const Offset(0, _floor),
              const [Color(0xFF1E140E), Color(0xFF35241A)]));
    canvas.drawRRect(
        r, Paint()..color = Color.fromRGBO(255, 200, 120, .22 * s.door));
    // ichkaridan polga tushgan iliq yorug'lik
    canvas.drawPath(
        Path()
          ..moveTo(_dl + 10, _floor)
          ..lineTo(_dr - 10, _floor)
          ..lineTo(_dr + 120, _floor + 40)
          ..lineTo(_dl + 30, _floor + 40)
          ..close(),
        Paint()..color = Color.fromRGBO(255, 200, 120, .14 * s.door));
    // ramka
    Path arch(double inset, double r) => Path()
      ..moveTo(_dl + inset, _floor)
      ..lineTo(_dl + inset, _dt + inset + r)
      ..arcToPoint(Offset(_dl + inset + r, _dt + inset), radius: Radius.circular(r))
      ..lineTo(_dr - inset - r, _dt + inset)
      ..arcToPoint(Offset(_dr - inset, _dt + inset + r), radius: Radius.circular(r))
      ..lineTo(_dr - inset, _floor);
    canvas.drawPath(
        arch(0, 22),
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 26
          ..color = _wood);
    final ink = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = _out
      ..strokeJoin = StrokeJoin.round
      ..color = _woodInk;
    canvas.drawPath(arch(-13, 35), ink);
    canvas.drawPath(arch(13, 12), ink);
    canvas.drawLine(
        const Offset(_dl - 2, _floor - 40),
        const Offset(_dl - 2, _dt + 40),
        Paint()
          ..strokeWidth = 6
          ..strokeCap = StrokeCap.round
          ..color = const Color(0x73FFD296));
  }

  /// Tabaqa chap ilmoqda ichkariga aylanadi (perspektiva: uzoq chet kichrayadi).
  void _doorPanel(Canvas canvas, _S s) {
    final phi = s.door * 1.45; // ~83°
    const x0 = _dl + 13, y0 = _dt + 13, y1 = _floor;
    const pw = (_dr - 13) - x0;
    final depth = math.sin(phi) * .22;
    final xf = x0 + pw * math.cos(phi);
    const yc = (y0 + y1) / 2;
    final hh = (y1 - y0) / 2 * (1 - depth);
    if ((xf - x0).abs() < 2) return;
    canvas.save();
    canvas.clipRect(const Rect.fromLTWH(_dl + 13, 0, _w, _h));
    final quad = Path()
      ..moveTo(x0, y0)
      ..lineTo(xf, yc - hh)
      ..lineTo(xf, yc + hh)
      ..lineTo(x0, y1)
      ..close();
    final shade = math.min(1.0, s.door * 1.2);
    canvas.drawPath(
        quad,
        Paint()
          ..shader = ui.Gradient.linear(Offset(x0, 0), Offset(xf, 0), [
            Color.lerp(const Color(0xFFD98A45), const Color(0xFF8F4A18), shade)!,
            Color.lerp(const Color(0xFFC7773A), const Color(0xFF6F3810), shade)!,
          ]));
    Offset at(double u, double v) {
      final x = _lerp(x0, xf, u);
      return Offset(x, _lerp(_lerp(y0, yc - hh, u), _lerp(y1, yc + hh, u), v));
    }

    final panelInk = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 7
      ..strokeJoin = StrokeJoin.round
      ..color = const Color(0xE66D3605);
    for (final v in const [
      [.08, .42],
      [.52, .92]
    ]) {
      final p = Path()
        ..moveTo(at(.16, v[0]).dx, at(.16, v[0]).dy)
        ..lineTo(at(.84, v[0]).dx, at(.84, v[0]).dy)
        ..lineTo(at(.84, v[1]).dx, at(.84, v[1]).dy)
        ..lineTo(at(.16, v[1]).dx, at(.16, v[1]).dy)
        ..close();
      canvas.drawPath(p, Paint()..color = const Color(0x1A000000));
      canvas.drawPath(p, panelInk);
    }
    canvas.drawLine(
        at(.07, .12),
        at(.07, .35),
        Paint()
          ..strokeWidth = 7
          ..strokeCap = StrokeCap.round
          ..color = Color.fromRGBO(255, 220, 170, .5 * (1 - shade)));
    final knob = at(.86, .55);
    final kr = Rect.fromCenter(
        center: knob,
        width: 2 * (12 * math.cos(phi).abs() + 3),
        height: 24 * (1 - depth * .5));
    canvas.drawOval(kr, Paint()..color = _gold);
    canvas.drawOval(
        kr,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 6
          ..color = _woodInk);
    canvas.drawPath(
        quad,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = _out
          ..strokeJoin = StrokeJoin.round
          ..color = _woodInk);
    canvas.restore();
  }

  // ── AXLAT QUTISI ───────────────────────────────────────────────

  void _binShadow(Canvas canvas) {
    canvas.drawOval(
        Rect.fromCenter(center: const Offset((_bl + _br) / 2, _floor + 4), width: 220, height: 30),
        Paint()..color = const Color(0x47000000));
  }

  void _bin(Canvas canvas, _S s) {
    const cx = (_bl + _br) / 2;
    final ink = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = _out
      ..strokeJoin = StrokeJoin.round
      ..color = _binInk;
    final body = Path()
      ..moveTo(_bl, _bt)
      ..lineTo(_br, _bt)
      ..lineTo(_br - 16, _floor - 18)
      ..quadraticBezierTo(_br - 19, _floor, _br - 38, _floor)
      ..lineTo(_bl + 38, _floor)
      ..quadraticBezierTo(_bl + 19, _floor, _bl + 16, _floor - 18)
      ..close();
    canvas.drawPath(
        body,
        Paint()
          ..shader = ui.Gradient.linear(const Offset(_bl, 0), const Offset(_br, 0),
              const [Color(0xFF58B3C9), Color(0xFF3F97AD), Color(0xFF2E7B8F)], const [0, .55, 1]));
    canvas.drawPath(body, ink);
    final rib = Paint()
      ..strokeWidth = 7
      ..strokeCap = StrokeCap.round
      ..color = const Color(0xBF1B4D59);
    for (final f in const [.3, .5, .7]) {
      final x = _lerp(_bl + 22, _br - 22, f);
      canvas.drawLine(Offset(x, _bt + 34), Offset(_lerp(x, cx, .08), _floor - 30), rib);
    }
    canvas.drawLine(
        const Offset(_bl + 22, _bt + 30),
        const Offset(_bl + 30, _bt + 110),
        Paint()
          ..strokeWidth = 8
          ..strokeCap = StrokeCap.round
          ..color = const Color(0x8CFFFFFF));
    // og'iz gardishi
    final rim = RRect.fromLTRBR(_bl - 10, _bt - 8, _br + 10, _bt + 14, const Radius.circular(11));
    canvas.drawRRect(rim, Paint()..color = const Color(0xFF2E7B8F));
    canvas.drawRRect(rim, ink);
    // qopqoq: chap-orqa ilmoqda ochiladi
    canvas.save();
    canvas.translate(_bl - 8, _bt - 6);
    canvas.rotate(-2.0 * s.lid);
    const lw = _br - _bl + 16;
    final lid = Path()
      ..moveTo(0, 0)
      ..lineTo(lw, 0)
      ..quadraticBezierTo(lw, -30, lw / 2, -34)
      ..quadraticBezierTo(0, -30, 0, 0)
      ..close();
    canvas.drawPath(
        lid,
        Paint()
          ..shader = ui.Gradient.linear(const Offset(0, -34), Offset.zero,
              const [Color(0xFF6CC6DB), Color(0xFF3F97AD)]));
    canvas.drawPath(lid, ink);
    canvas.drawRRect(RRect.fromLTRBR(lw / 2 - 22, -50, lw / 2 + 22, -32, const Radius.circular(9)),
        Paint()..color = _binInk);
    canvas.drawPath(
        Path()
          ..moveTo(22, -12)
          ..quadraticBezierTo(40, -24, 62, -26),
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 7
          ..strokeCap = StrokeCap.round
          ..color = const Color(0x99FFFFFF));
    canvas.restore();
  }

  // ── CHANG VA UCHQUNLAR ─────────────────────────────────────────

  void _puff(Canvas canvas, _S s) {
    if (s.puff <= 0 || s.puff >= 1) return;
    final a = 1 - _eio(s.puff);
    final k = _eout(s.puff);
    const cx = (_bl + _br) / 2;
    // Bitta qatlamda: bulutlar bir-birining ustida qorayib qolmasin.
    canvas.saveLayer(null, Paint()..color = Color.fromRGBO(0, 0, 0, .9 * a));
    final p = Paint()..color = _dust;
    for (final d in const [
      [-70.0, -10.0, 22.0, 1.0],
      [-35.0, -38.0, 28.0, .8],
      [8.0, -44.0, 26.0, .9],
      [48.0, -30.0, 24.0, 1.0],
      [80.0, -6.0, 18.0, 1.1],
    ]) {
      canvas.drawCircle(
          Offset(cx + d[0] * (1 + k * .7 * d[3]), _bt - 16 + d[1] * (.6 + k * .9)),
          d[2] * (.5 + k * .8) * (1 - .3 * s.puff),
          p);
    }
    canvas.restore();
  }

  void _sparks(Canvas canvas, _S s) {
    if (s.spark <= 0 || s.spark >= 1) return;
    const pts = [
      [_bl - 20, _bt - 120, 1.0],
      [_br + 10, _bt - 90, .8],
      [(_bl + _br) / 2 + 20, _bt - 170, 1.1],
    ];
    final paint = Paint()..color = _gold;
    for (var i = 0; i < pts.length; i++) {
      final u = _clamp(s.spark * 1.4 - i * .18, 0, 1);
      if (u <= 0 || u >= 1) continue;
      final r = math.sin(u * math.pi) * 22 * pts[i][2];
      canvas.save();
      canvas.translate(pts[i][0], pts[i][1] - u * 30);
      canvas.rotate(u * 1.5);
      final star = Path();
      for (var j = 0; j < 8; j++) {
        final ang = j * math.pi / 4;
        final rr = j.isOdd ? r * .35 : r;
        final pt = Offset(math.cos(ang) * rr, math.sin(ang) * rr);
        if (j == 0) {
          star.moveTo(pt.dx, pt.dy);
        } else {
          star.lineTo(pt.dx, pt.dy);
        }
      }
      star.close();
      canvas.drawPath(star, paint);
      canvas.restore();
    }
  }

  @override
  bool shouldRepaint(_ScenePainter old) => old.anim != anim;
}
