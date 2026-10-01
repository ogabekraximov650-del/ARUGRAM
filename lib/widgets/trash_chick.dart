// lib/widgets/trash_chick.dart — "KESH TOZALANMOQDA" ANIMATSIYASI.
//
// TALAB (foydalanuvchi): Telegram jo'jasi (`utyan`) uyining eshigidan chiqib
// chiqindini axlat qutisiga tashlaydi; chiqindi HAJMIGA qarab sahna boshqacha:
//
//   1. 500 MB gacha — jo'ja eshikdan CHIQMAYDI: o'zining chap qo'li bilan
//      eshik ramkasining o'ziga nisbatan chap tomonini ushlab, o'ngga va
//      chapga mo'ralaydi, hech kim yo'q — o'ng qo'li bilan kichik qopni shu
//      yerdan qutiga otadi (qop qutiga tushadi).
//   2. 500 MB – 2 GB — qopni qo'lida qutigacha olib borib, ichiga tashlaydi.
//   3. 2 – 5 GB — qop qutidan katta: og'zidagi tugunidan ushlab, orqasi bilan
//      yurib qiynalib sudraydi (qop tubi yerda, egilib sudraladi) va qutining
//      yoniga qo'yadi, peshanasidagi terni artadi.
//   4. 5 GB dan katta — qop eshikka zo'rg'a sig'adi: jo'ja chiranib tortadi,
//      qop birdan otilib chiqadi, jo'ja orqaga uchib o'tirib qoladi va ikki
//      qo'li bilan ko'zini ishqalab yig'laydi; yig'lab, qiynalib qopni
//      qutining yoniga sudrab boradi va yig'lagancha uyiga kirib ketadi.
//
// Sahna: chapda uyning bir burchagi (tom, devor, eshik), undan o'ng chetgacha
// yog'och panjara, o'ngda og'zi doim ochiq axlat qutisi. Hammasi kichik —
// eshikdan qutigacha uzoq yo'l.
//
// Jo'ja — `utyan_cache.json` dagi asl vektor shakllar (`utyan_parts.dart`),
// burilishi Lottie'ning o'z holatlari aralashmasi (qog'ozdek yassilanmaydi).
// Qo'llar — har biri alohida va kalta (yelkadan panjagacha), bir-biriga bog'liq emas.
// Animatsiya BIR marta o'ynaydi; uzunligi — [TrashChickAnimation.durationFor].
// Sahna 1360 x 680 birlikda yozilgan va berilgan o'lchamga sig'diriladi.

import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import 'utyan_parts.dart';

class TrashChickAnimation extends StatefulWidget {
  /// Tozalanayotgan chiqindi hajmi (bayt) — sahna va qop o'lchami shunga qarab.
  final int bytes;
  final double width;
  final double height;
  const TrashChickAnimation(
      {super.key, required this.bytes, this.width = 340, this.height = 170});

  /// Shu hajm uchun animatsiya qancha davom etadi.
  static Duration durationFor(int bytes) =>
      Duration(milliseconds: (_durationOf(_tierOf(bytes)) * 1000).round());

  @override
  State<TrashChickAnimation> createState() => _TrashChickAnimationState();
}

class _TrashChickAnimationState extends State<TrashChickAnimation>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
      vsync: this, duration: TrashChickAnimation.durationFor(widget.bytes))
    ..forward();

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
        painter: _ScenePainter(_c, widget.bytes),
      ),
    );
  }
}

// ── SAHNA O'LCHAMLARI ─────────────────────────────────────────────
const double _w = 1360, _h = 680, _floor = 600;
const double _wallR = 262; // uy devorining o'ng cheti
const double _dl = 78, _dr = 222, _dt = 318; // eshik o'yig'i
const double _dc = (_dl + _dr) / 2;
const double _bl = 1186, _br = 1300, _bt = 462; // quti (og'zi ochiq)
const double _bc = (_bl + _br) / 2;
const double _k = 0.40; // jo'ja masshtabi (512 -> sahna)
const double _hw = 306 * _k / 2; // jo'janing yarim eni
const double _px = 294, _py = 392, _cx = 300, _headC = 302;
const double _stopX = _bl - _hw + 6; // qutigacha yetib to'xtash
const double _dragX = _bl - 18 + _hw; // sudralgan qop qutining yonida
const int _mb = 1024 * 1024;

// ── YUMSHATISH ───────────────────────────────────────────────────
double _clamp(double v, double a, double b) => math.max(a, math.min(b, v));
double _seg(double t, double a, double b) => _clamp((t - a) / (b - a), 0, 1);
double _lerp(double a, double b, double t) => a + (b - a) * t;
double _eio(double x) =>
    x < .5 ? 4 * x * x * x : 1 - math.pow(-2 * x + 2, 3).toDouble() / 2;
double _eout(double x) => 1 - math.pow(1 - x, 3).toDouble();
double _sio(double x) => -(math.cos(math.pi * x) - 1) / 2;
double _backOut(double x, [double s = 1.7]) =>
    1 +
    (s + 1) * math.pow(x - 1, 3).toDouble() +
    s * math.pow(x - 1, 2).toDouble();

int _tierOf(int bytes) => bytes < 500 * _mb
    ? 1
    : (bytes < 2048 * _mb ? 2 : (bytes < 5120 * _mb ? 3 : 4));

double _durationOf(int tier) => const [0.0, 7.0, 13.5, 16.5, 22.5][tier];

/// Qop balandligi (sahna birligida), eni = 0.82 * balandlik.
double _bagSize(int bytes) {
  final g = bytes / _mb;
  if (g < 500) return _lerp(34, 52, _clamp(g / 500, 0, 1));
  if (g < 2048) return _lerp(58, 92, _clamp((g - 500) / 1548, 0, 1));
  if (g < 5120) return _lerp(158, 190, _clamp((g - 2048) / 3072, 0, 1));
  return 290;
}

class _Walk {
  final double x, ph;
  const _Walk(this.x, this.ph);
}

/// [t0, t1] da x0 -> x1, qadam uzunligi `stride` (ph < 0 — yurmayapti).
_Walk _walk(double t, double t0, double t1, double x0, double x1, double stride) {
  final u = _seg(t, t0, t1);
  final steps = math.max(1, ((x1 - x0).abs() / stride).round());
  return _Walk(_lerp(x0, x1, u), u > 0 && u < 1 ? u * steps : -1);
}

enum _Eyes { calm, happy, strain, cry }

enum _BagMode { hand, fly, bin, drag }

enum _Pass { inside, outside, all }

// ── VAQT JADVALI ─────────────────────────────────────────────────
class _S {
  final int tier;
  final double sec, bh, bw;
  double door = 0, x = _dc, yaw = 0, ph = -1, stride = 1, lean = 0, lift = 0;
  double sx = 1, sy = 1, rot = 0, sweat = 0, shake = 0, tears = 0;
  double wind = 0, fling = 0, grip = 0, hop = 0, flail = 0, wipeBrow = 0;
  double puff = -1, spark = -1, binWob = 0, flyT0 = 0, flyT1 = 0, flyPeak = 0;
  bool pivotR = false, happy = false, dragging = false, stuck = false;
  bool holding = false, sob = false, rubEye = false, rubBoth = false;
  double flyBack = 0, walkOut = 0, jerk = 0, dragU = 0, dropAt = 99;
  String? throwArm; // 'a' / 'b' — otadigan qo'l (berilmasa — yaqini)
  _Eyes eyes = _Eyes.calm;
  _BagMode bag = _BagMode.hand;
  // sudralayotgan qop: tubi (px) yerda, burchagi (ang), siqilishi (sq)
  double bagPx = double.nan, bagAng = 0, bagSq = 1;
  Offset? knot; // qop uchidagi tugun (sahnada) — ikki qo'l shu yerni ushlaydi

  _S(int bytes, this.sec)
      : tier = _tierOf(bytes),
        bh = _bagSize(bytes),
        bw = _bagSize(bytes) * .82 {
    final t = sec;
    switch (tier) {
      case 1:
        _tier1(t);
      case 2:
        _tier2(t);
      case 3:
        _tier3(t);
      default:
        _tier4(t);
    }
  }

  void _tier1(double t) {
    door = _eio(_seg(t, 0, .7)) - _eio(_seg(t, 6.1, 6.9));
    x = _dc + 18; // o'ng (o'ziga chap) ustunga yaqin turadi
    throwArm = 'a'; // o'ng qo'li (chap qo'li ramkada)
    var y = _lerp(0, 1, _eio(_seg(t, .85, 1.15)));
    y = _lerp(y, -1, _eio(_seg(t, 1.95, 2.35)));
    y = _lerp(y, 1, _eio(_seg(t, 2.85, 3.15)));
    y = _lerp(y, 0, _eio(_seg(t, 5.55, 5.85)));
    yaw = y;
    // eshikdan tashqariga engashib mo'ralaydi
    lean = .13 * math.sin(math.pi * _seg(t, 1.1, 1.95)) -
        .05 * math.sin(math.pi * _seg(t, 2.3, 2.85));
    grip = _eio(_seg(t, .75, 1.05)) - _eio(_seg(t, 4.75, 5.0));
    wind = _sio(_seg(t, 3.2, 3.6)) * (1 - _seg(t, 3.6, 3.68));
    fling = _eout(_seg(t, 3.6, 3.72)) * (1 - _sio(_seg(t, 4.0, 4.4)));
    flyT0 = 3.68;
    flyT1 = 4.85;
    flyPeak = 300;
    if (t >= flyT0) bag = t < flyT1 ? _BagMode.fly : _BagMode.bin;
    binWob = t > flyT1
        ? math.exp(-(t - flyT1) * 5) * math.sin((t - flyT1) * 28)
        : 0.0;
    puff = _seg(t, 4.85, 5.5);
    spark = _seg(t, 4.9, 5.8);
    hop = t > 4.95 && t < 5.5 ? math.sin(_seg(t, 4.95, 5.5) * math.pi) : 0.0;
    happy = t > 4.9 && t < 5.7;
    eyes = happy ? _Eyes.happy : _Eyes.calm;
  }

  void _tier2(double t) {
    door = _eio(_seg(t, 0, .7)) - _eio(_seg(t, 12.6, 13.4));
    var y = _lerp(0, 1, _eio(_seg(t, .55, .85)));
    y = _lerp(y, 0, _eio(_seg(t, 7.0, 7.25)));
    y = _lerp(y, -1, _eio(_seg(t, 7.9, 8.2)));
    y = _lerp(y, 0, _eio(_seg(t, 12.35, 12.55)));
    yaw = y;
    var w = _walk(t, .75, 5.6, _dc, _stopX - 60, 58);
    x = w.x;
    ph = w.ph;
    wind = _sio(_seg(t, 5.75, 6.15)) * (1 - _seg(t, 6.15, 6.22));
    fling = _eout(_seg(t, 6.15, 6.27)) * (1 - _sio(_seg(t, 6.6, 7.0)));
    flyT0 = 6.22;
    flyT1 = 6.85;
    flyPeak = 120;
    if (t >= flyT0) bag = t < flyT1 ? _BagMode.fly : _BagMode.bin;
    binWob = t > flyT1
        ? math.exp(-(t - flyT1) * 5) * math.sin((t - flyT1) * 28)
        : 0.0;
    puff = _seg(t, 6.85, 7.5);
    spark = _seg(t, 6.9, 7.8);
    hop = t > 7.1 && t < 7.75 ? math.sin(_seg(t, 7.1, 7.75) * math.pi) : 0.0;
    happy = t > 6.9 && t < 7.9;
    eyes = happy ? _Eyes.happy : _Eyes.calm;
    if (t > 8.2) {
      w = _walk(t, 8.2, 12.35, _stopX - 60, _dc, 58);
      x = w.x;
      ph = w.ph;
    }
  }

  void _tier3(double t) {
    door = _eio(_seg(t, 0, .7)) - _eio(_seg(t, 15.6, 16.4));
    // orqasi bilan (uyga qarab) qopni tugunidan sudraydi
    var y = -1.0;
    y = _lerp(y, 0, _eio(_seg(t, 9.0, 9.35)));
    y = _lerp(y, -1, _eio(_seg(t, 10.6, 10.9)));
    y = _lerp(y, 0, _eio(_seg(t, 15.3, 15.5)));
    yaw = y;
    var w = _walk(t, .7, 8.6, _dc, _dragX, 34);
    x = w.x;
    ph = w.ph;
    stride = .6;
    dragging = t < 8.6;
    lean = dragging ? .16 + .03 * math.sin(t * 9) : 0.0;
    eyes = dragging ? _Eyes.strain : _Eyes.calm;
    sweat = dragging ? 1.0 : math.max(0.0, 1 - (t - 8.7) * 1.5);
    bag = _BagMode.drag;
    dropAt = 8.6; // qo'yib yuborilgach yonboshlab yotadi
    // yengil nafas va peshanadagi terni artish
    if (t > 8.7 && t < 10.4) {
      final u = _seg(t, 8.7, 10.4);
      sy = 1 - .06 * math.sin(u * math.pi);
      sx = 1 + .05 * math.sin(u * math.pi);
      wipeBrow = math.sin(u * math.pi);
    }
    if (t > 10.9) {
      w = _walk(t, 10.9, 15.3, _dragX, _dc, 58);
      x = w.x;
      ph = w.ph;
      stride = 1;
    }
  }

  void _tier4(double t) {
    // 5 GB+: qop eshikka tiqiladi, jo'ja chiranib tortadi, qop otilib chiqadi,
    // jo'ja orqaga uchib o'tirib qoladi va yig'laydi, yig'lab sudrab boradi.
    // Jo'ja va qopning aniq o'rni — `_ScenePainter._resolve` da.
    door = _eio(_seg(t, 0, .8)) - _eio(_seg(t, 21.6, 22.4));
    bag = _BagMode.drag;
    var y = -1.0;
    y = _lerp(y, 0, _eio(_seg(t, 4.6, 5.0))); // yiqilganda yuzi oldinga
    y = _lerp(y, -1, _eio(_seg(t, 8.0, 8.3))); // qopga qaraydi
    y = _lerp(y, 0, _eio(_seg(t, 16.2, 16.5)));
    y = _lerp(y, -1, _eio(_seg(t, 17.4, 17.7)));
    y = _lerp(y, 0, _eio(_seg(t, 21.3, 21.5)));
    yaw = y;
    if (t < 4.45) {
      // eshikda tiqilgan; jo'ja silkinib tortadi
      jerk = math.max(0.0, math.sin(_seg(t, 1.8, 4.45) * math.pi * 6)) *
          _seg(t, 1.8, 4.45);
      walkOut = _seg(t, .8, 1.8);
      ph = _walk(t, .8, 1.8, _dc, _dc + 200, 30).ph;
      stride = .6;
      lean = t < 1.8 ? .12 : .3 + .08 * jerk;
      shake = t > 1.8 ? 1.0 : 0.0;
      eyes = t > 1.8 ? _Eyes.strain : _Eyes.calm;
      sweat = t > 2.0 ? 1.0 : 0.0;
      stuck = true;
      dragging = true;
      bagSq = .84;
      return;
    }
    // jo'ja orqaga uchadi va o'tirib qoladi
    final f = _seg(t, 4.45, 5.0), land = _seg(t, 5.0, 5.45);
    flyBack = _eout(f);
    lift = math.sin(f * math.pi) * 40;
    pivotR = true;
    var r = .42 * _eout(f);
    r = _lerp(r, .24, _eout(land));
    r = _lerp(r, 0, _eio(_seg(t, 7.4, 7.9))); // turadi
    rot = r;
    if (t > 5.0 && t < 5.5) {
      final kq = math.sin(land * math.pi);
      sy *= 1 - .16 * kq;
      sx *= 1 + .12 * kq;
    }
    eyes = t < 5.05 ? _Eyes.strain : _Eyes.cry;
    sweat = t < 5.05 ? 1.0 : 0.0;
    sob = t > 5.3 && t < 7.4;
    flail = t < 5.1 ? math.sin(_seg(t, 4.45, 5.1) * math.pi) : 0.0;
    rubEye = (t > 5.4 && t < 7.5) || (t > 16.0 && t < 21.3);
    rubBoth = t < 7.5;
    tears = t > 5.2 ? 1.0 : 0.0;
    // tugunga yetib boradi va yig'lab, chiranib sudraydi
    if (t >= 8.4) {
      dragU = _seg(t, 8.4, 16.0);
      ph = _walk(t, 8.4, 16.0, 0, 700, 28).ph;
      stride = .5;
      dragging = t < 16.0;
      lean = dragging ? .22 + .04 * math.sin(t * 8) : 0.0;
      shake = dragging ? .4 : 0.0;
    }
    holding = t >= 8.0 && t < 16.0;
    if (t > 16.0 && t < 17.4) {
      final v = _seg(t, 16, 17.4);
      sy = 1 - .07 * math.sin(v * math.pi);
      sx = 1 + .05 * math.sin(v * math.pi);
      sob = true;
    }
    dropAt = 16.0;
    if (t > 17.7) {
      final w3 = _walk(t, 17.7, 21.3, _dragX, _dc, 40);
      x = w3.x;
      ph = w3.ph;
      stride = .75;
    }
  }
}

// ── 2D AFFIN ─────────────────────────────────────────────────────
class _Aff {
  double a = 1, b = 0, c = 0, d = 1, e = 0, f = 0;
  _Aff translate(double x, double y) {
    e += a * x + c * y;
    f += b * x + d * y;
    return this;
  }

  _Aff scale(double sx, double sy) {
    a *= sx;
    b *= sx;
    c *= sy;
    d *= sy;
    return this;
  }

  _Aff rotate(double r) {
    final co = math.cos(r), si = math.sin(r);
    final na = a * co + c * si, nb = b * co + d * si;
    final nc = -a * si + c * co, nd = -b * si + d * co;
    a = na;
    b = nb;
    c = nc;
    d = nd;
    return this;
  }

  Offset map(Offset p) =>
      Offset(a * p.dx + c * p.dy + e, b * p.dx + d * p.dy + f);

  Offset unmap(Offset p) {
    final det = a * d - b * c;
    final x = p.dx - e, y = p.dy - f;
    return Offset((d * x - c * y) / det, (-b * x + a * y) / det);
  }

  Float64List get storage => Float64List.fromList(
      [a, b, 0, 0, c, d, 0, 0, 0, 0, 1, 0, e, f, 0, 1]);
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

List<double> _pose(String name, double yaw) {
  final part = _parts[name]!;
  if (yaw >= 0) return _mix(part.a, part.b, yaw);
  final u = -yaw;
  final ra = _bbox(part.a);
  final swap = _faceSwap[name];
  final Rect rb;
  final double tx;
  if (swap != null) {
    rb = _bbox(_parts[swap]!.b);
    tx = 2 * _headC - rb.center.dx;
  } else {
    rb = _bbox(part.b);
    tx = ra.center.dx - (rb.center.dx - ra.center.dx);
  }
  return _xform(part.a, ra.center.dx, _lerp(1, rb.width / ra.width, u),
      (tx - ra.center.dx) * u, (rb.center.dy - ra.center.dy) * u);
}

// ── QO'LLAR: har biri alohida (yelkadan panjagacha) ─────────────────
const double _shY = 250, _shDx = 96, _armL = 62; // qo'llar kalta

class _Arms {
  final Offset shA, shB, a, b;
  final double face;
  const _Arms(this.shA, this.shB, this.a, this.b, this.face);
}

Offset _polar(Offset p, double deg, double l) => Offset(
    p.dx + math.cos(deg * math.pi / 180) * l,
    p.dy + math.sin(deg * math.pi / 180) * l);

Offset _lerp2(Offset a, Offset b, double t) => Offset.lerp(a, b, t)!;

Offset _capTo(Offset sh, Offset p, double maxL) {
  final d = (p - sh).distance;
  if (d <= maxL || d == 0) return p;
  return sh + (p - sh) * (maxL / d);
}

// ── RASSOM ───────────────────────────────────────────────────────
class _ScenePainter extends CustomPainter {
  final Animation<double> anim;
  final int bytes;
  _ScenePainter(this.anim, this.bytes) : super(repaint: anim);

  double get _dur => _durationOf(_tierOf(bytes));

  @override
  void paint(Canvas canvas, Size size) {
    final k = math.min(size.width / _w, size.height / _h);
    canvas.save();
    canvas.translate((size.width - _w * k) / 2, (size.height - _h * k) / 2);
    canvas.scale(k);
    canvas.clipRect(const Rect.fromLTWH(0, 0, _w, _h));
    final s = _resolve(bytes, _S(bytes, anim.value * _dur));
    Offset? release;
    if (s.bag == _BagMode.fly || s.bag == _BagMode.bin) {
      release = _handWorld(_S(bytes, s.flyT0 - 0.0001));
    }
    // uy ichidami (devor orqasida chiziladi)
    final chickIn = s.x <= _dc + 20 && s.door < .9;
    // qop eshikdan o'tayotgan bo'lsa: ichkaridagi qismi devor orqasida
    final bagSplit =
        s.bag == _BagMode.drag && !s.bagPx.isNaN && s.bagPx < _dr + 40;
    final overBin = s.x > _bl - _hw;

    _ground(canvas);
    _fence(canvas);
    _houseBack(canvas, s);
    _binBack(canvas, s);
    if (bagSplit) _dragBag(canvas, s, _Pass.inside);
    if (chickIn) _chick(canvas, s);
    _doorPanel(canvas, s);
    _houseFront(canvas);
    _dragBag(canvas, s, bagSplit ? _Pass.outside : _Pass.all);
    if (!chickIn && !overBin) _chick(canvas, s);
    // eshik ramkasini ORQASIDAN ushlagan (chap) qo'l: tananing oldida,
    // qanot uchi esa ustun orqasiga kirib turadi
    if (s.grip > .5) {
      final m = _matrix(s);
      final ar = _arms(s, m);
      canvas.save();
      canvas.clipRect(const Rect.fromLTRB(0, 0, _dr, _h));
      canvas.transform(m.storage);
      _arm(canvas, ar.shB, ar.b);
      canvas.restore();
    }
    if (release != null) _flying(canvas, s, release);
    _binFront(canvas, s);
    if (overBin) _chick(canvas, s);
    _puff(canvas, s);
    _sparks(canvas, s);
    canvas.restore();
  }

  // ── SUDRALAYOTGAN QOP ───────────────────────────────────────────
  //
  // Katta qop TO'LIQ YOTGAN holda sudraladi: tubi orqada, uchidagi tuguni
  // jo'janing IKKI qo'lida. Qo'llar tanadan `_hOff` oldinda.
  static const double _hOff = _hw * .55;

  /// Qop tubidan tugunigacha masofa.
  static double _bagLen(_S s) => .9 * s.bh * (1 + (1 - s.bagSq) * .35);

  static double _knotY(_S s) => _floor - s.bw * s.bagSq / 2 * .85 + 2;

  static void _setKnot(_S s, double knotX) {
    s.bagAng = math.pi / 2;
    s.bagPx = knotX - _bagLen(s);
    s.knot = Offset(knotX, _knotY(s));
  }

  static _S _resolve(int bytes, _S s) {
    if (s.bag != _BagMode.drag) return s;
    final t = s.sec;
    if (s.tier == 3) {
      _setKnot(s, (t < s.dropAt ? s.x : _dragX) - _hOff);
      if (t >= s.dropAt) s.knot = null;
      return s;
    }
    // 5 GB+
    const stuckPx = _dl + 10;
    if (t < 4.45) {
      final xStuck = stuckPx + _bagLen(s) + _hOff;
      s.x = _lerp(_dc, xStuck, _eio(s.walkOut)) + s.jerk * 10;
      _setKnot(s, s.x - _hOff);
      s.bagPx = math.min(s.bagPx, stuckPx) + s.jerk * 6;
      s.knot = Offset(s.bagPx + _bagLen(s), _knotY(s));
      return s;
    }
    final p = _resolve(bytes, _S(bytes, 4.45 - 1e-4));
    final pu = _seg(t, 4.45, 4.8);
    s.bagSq = _lerp(.84, 1, _backOut(math.min(1.0, pu * 1.4), 2.5));
    final restPx = p.bagPx + 170;
    s.bagAng = math.pi / 2;
    s.bagPx = _lerp(p.bagPx, restPx, _eout(pu));
    s.knot = null;
    final xLand = p.x + 140;
    s.x = _lerp(p.x, xLand, s.flyBack);
    if (t >= 8.0) {
      final xGrab = restPx + _bagLen(s) + _hOff;
      if (t < 8.4) {
        s.x = _lerp(xLand, xGrab, _eio(_seg(t, 8.0, 8.4)));
        s.knot = Offset(restPx + _bagLen(s), _knotY(s));
      } else {
        s.x = _lerp(xGrab, _dragX, s.dragU);
        _setKnot(s, (t < s.dropAt ? s.x : _dragX) - _hOff);
        if (t >= s.dropAt) s.knot = null;
      }
    }
    if (t > 17.7) s.x = _walk(t, 17.7, 21.3, _dragX, _dc, 40).x;
    return s;
  }

  // ── JO'JA ──────────────────────────────────────────────────────

  static _Aff _matrix(_S s) {
    var rot = s.rot, lift = s.lift, sx = s.sx, sy = s.sy;
    if (s.ph >= 0) {
      final ph = s.ph % 1, n = s.ph.floor();
      lift += math.sin(ph * math.pi) * 16 * s.stride;
      rot += (n.isOdd ? 1.0 : -1.0) * math.sin(ph * math.pi) * .06 * s.stride;
      final land =
          ph < .15 ? 1 - ph / .15 : (ph > .85 ? (ph - .85) / .15 : 0.0);
      sy *= 1 - .06 * land;
      sx *= 1 + .05 * land;
    }
    final dir = s.yaw < 0 ? -1.0 : 1.0;
    if (s.wind > 0 || s.fling > 0) {
      sy *= 1 - .08 * s.wind + .06 * s.fling;
      sx *= 1 + .07 * s.wind - .04 * s.fling;
      rot += (-.10 * s.wind + .12 * s.fling) * dir;
    }
    if (s.hop > 0) {
      lift += s.hop * 46;
      sy *= 1 + .06 * s.hop;
      sx *= 1 - .04 * s.hop;
    }
    rot += s.lean; // qopni tortganda orqaga og'adi
    if (s.shake > 0) rot += math.sin(s.sec * 55) * .025 * s.shake;
    if (s.sob) sy *= 1 - .025 * math.sin(s.sec * 14).abs();
    final m = _Aff();
    if (s.pivotR) {
      m.translate(s.x + _hw, _floor - lift).rotate(rot).translate(-_hw, 0);
    } else {
      m.translate(s.x, _floor - lift).rotate(rot);
    }
    m.scale(_k * sx, _k * sy).translate(-_px, -_py);
    return m;
  }

  /// Qo'l nuqtalari (jo'janing 512 fazosida).
  static _Arms _arms(_S s, _Aff m) {
    final y = s.yaw;
    final kk = 1 - .5 * y.abs(), off = 34 * y;
    final shA = Offset(_cx - _shDx * kk + off, _shY);
    final shB = Offset(_cx + _shDx * kk + off, _shY);
    final face = y < -.05 ? -1.0 : 1.0;
    double mir(double d) => face > 0 ? d : 180 - d;
    var aA = 115.0, aB = 65.0;
    if (s.ph >= 0) {
      final ph = s.ph % 1, n = s.ph.floor();
      final sw = math.sin(ph * math.pi) * (n.isOdd ? 1 : -1) * 24;
      aA += sw;
      aB -= sw;
    }
    var a = _polar(shA, aA, _armL), b = _polar(shB, aB, _armL);
    final nearIsB = face > 0;
    final throwB = s.throwArm != null ? s.throwArm == 'b' : nearIsB;
    // qop qo'lda: yerga tegmasin; otishda orqaga, so'ng yuqori-oldinga
    if (s.bag == _BagMode.hand || s.fling > 0) {
      final s0 = throwB ? shB : shA;
      final hy = math.min(320.0, _py - .9 * s.bh / _k - 26);
      final hold = _capTo(s0, Offset(s0.dx + face * 58, hy), _armL * 1.25);
      final back = _polar(s0, mir(160), _armL * .95);
      final up = _polar(s0, mir(-55), _armL);
      var h = _lerp2(hold, back, s.wind);
      h = _lerp2(h, up, s.fling);
      if (throwB) {
        b = h;
      } else {
        a = h;
      }
    }
    // eshik ramkasini o'zining CHAP qo'li (b) bilan, o'ziga nisbatan chap
    // tomonidan (bizga o'ng ustun) ushlaydi (500 MB gacha)
    if (s.grip > 0) {
      final p = m.unmap(const Offset(_dr + 8, _floor - 62));
      b = _lerp2(b, _capTo(shB, p, _armL * 2.2), s.grip);
    }
    if (s.happy) {
      final w = math.sin(s.sec * 22) * 14;
      a = _polar(shA, -125 + w, _armL);
      b = _polar(shB, -55 - w, _armL);
    }
    // sudrash / tortish: ikkala qo'l qop tomonga, titraydi
    // sudrash / tortish: ikkala qo'l bilan qop uchidagi tugunni ushlaydi
    final knot = s.knot;
    if ((s.dragging || s.stuck || s.holding) && knot != null) {
      final tr = s.shake * math.sin(s.sec * 50) * 5;
      final k = m.unmap(knot);
      a = _capTo(shA, Offset(k.dx + 8, k.dy + 12 + tr), _armL * 1.9);
      b = _capTo(shB, Offset(k.dx - 8, k.dy - 12 - tr), _armL * 1.9);
    }
    if (s.flail > 0) {
      final w = math.sin(s.sec * 30) * 30;
      a = _lerp2(a, _polar(shA, -140 + w, _armL), s.flail);
      b = _lerp2(b, _polar(shB, -40 - w, _armL), s.flail);
    }
    // ko'zini artib yig'laydi
    if (s.rubEye && !s.dragging) {
      final r = math.sin(s.sec * 16) * 10;
      final e1 = _bbox(_pose('eye', y)).center;
      final e2 = _bbox(_pose('eye_2', y)).center;
      final ha = _capTo(shA, Offset(e1.dx + r, e1.dy + 20), _armL * 1.7);
      final hb = _capTo(shB, Offset(e2.dx - r, e2.dy + 20), _armL * 1.7);
      if (s.rubBoth) {
        a = ha;
        b = hb;
      } else if (nearIsB) {
        b = hb;
      } else {
        a = ha;
      }
    }
    // peshanadagi terni artadi
    if (s.wipeBrow > 0) {
      final h = Offset(_headC + math.sin(s.sec * 10) * 30, 62);
      a = _lerp2(a, _capTo(shA, h, _armL * 1.8), s.wipeBrow);
    }
    return _Arms(shA, shB, a, b, face);
  }

  /// Qanot uchi (panja) — qop shu yerdan osilib turadi.
  static Offset _tip(Offset sh, Offset hand) {
    final dv = hand - sh;
    final d = dv.distance == 0 ? 1.0 : dv.distance;
    return sh + dv * ((d + 20) / d);
  }

  /// Qanot uchidan osilgan qopning markazi (tuguni — uchida).
  static Offset _bagAt(_S s, Offset tip) => Offset(tip.dx, tip.dy + .4 * s.bh / _k);

  /// Qop ko'taradigan qo'l: 'a' / 'b' (qop qo'lda bo'lmasa — null).
  static String? _carry(_S s, _Arms ar) => s.bag == _BagMode.hand || s.fling > 0
      ? (s.throwArm ?? (ar.face > 0 ? 'b' : 'a'))
      : null;

  static Offset _handWorld(_S s) {
    final m = _matrix(s);
    final ar = _arms(s, m);
    final k = _carry(s, ar) ?? (ar.face > 0 ? 'b' : 'a');
    final tip = k == 'b' ? _tip(ar.shB, ar.b) : _tip(ar.shA, ar.a);
    return m.map(_bagAt(s, tip));
  }

  /// Qanotsimon keng qo'l (asl jo'janing qo'llari kabi): ildizi yelkada,
  /// uchi — panja.
  void _arm(Canvas canvas, Offset sh, Offset hand) {
    final dv = hand - sh;
    final l = dv.distance + 24;
    canvas.save();
    canvas.translate(sh.dx, sh.dy);
    canvas.rotate(math.atan2(dv.dy, dv.dx));
    final p = Path()
      ..moveTo(-8, -30)
      ..quadraticBezierTo(l * .45, -36, l * .72, -26)
      ..cubicTo(l + 8, -18, l + 8, 18, l * .72, 24)
      ..quadraticBezierTo(l * .45, 32, -8, 30);
    canvas.drawPath(
        p,
        Paint()
          ..shader = ui.Gradient.linear(const Offset(0, -30), const Offset(0, 30),
              const [Color(0xFFFFE066), Color(0xFFFFC81F)]));
    Paint line(double w, Color c) => Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = w
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..color = c;
    canvas.drawPath(p, line(10, const Color(0xFFFA9016)));
    canvas.drawPath(
        Path()
          ..moveTo(l * .22, -17)
          ..quadraticBezierTo(l * .45, -22, l * .62, -16),
        line(8, const Color(0xE6FFFFFF)));
    canvas.restore();
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
    final m = _matrix(s);
    // soya
    final lift = s.lift +
        (s.ph >= 0 ? math.sin((s.ph % 1) * math.pi) * 16 : 0.0) +
        s.hop * 46;
    final lie = s.pivotR ? math.sin(s.rot) : 0.0;
    canvas.drawOval(
        Rect.fromCenter(
            center: Offset(s.x + lie * _hw, _floor + 3),
            width: 2 * (_hw * 1.05 * (1 - lift / 250) + lie * _hw * .6),
            height: 18),
        Paint()..color = Color.fromRGBO(0, 0, 0, _clamp(.3 - lift / 300, 0, 1)));

    canvas.save();
    canvas.transform(m.storage);
    final y = s.yaw;
    final ar = _arms(s, m);
    final near = ar.face > 0 ? 'b' : 'a';
    final carry = _carry(s, ar);
    // uzoqdagi qo'l — tana orqasida (qop ko'targan qo'l doim oldinda)
    final gripB = s.grip > .5; // ramkani ushlagan qo'l alohida chiziladi
    if (y.abs() > .35) {
      if (near == 'b' && carry != 'a') _arm(canvas, ar.shA, ar.a);
      if (near == 'a' && carry != 'b' && !gripB) _arm(canvas, ar.shB, ar.b);
    }
    for (final n in const [
      'body', 'head_bl3', 'head', 'head_bl1', 'head_bl2', 'beak', 'beak_bl',
      'mouth'
    ]) {
      _part(canvas, n, _pose(n, y));
    }
    // ko'zlar
    final e1 = _pose('eye', y), e2 = _pose('eye_2', y);
    final c1 = _bbox(e1).center, c2 = _bbox(e2).center;
    Paint ink(double w, [Color c = Colors.black]) => Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = w
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..color = c;
    switch (s.eyes) {
      case _Eyes.happy:
        _part(canvas, 'eye', _flipY(e1));
        _part(canvas, 'eye_2', _flipY(e2));
      case _Eyes.strain:
        // > <  qattiq yumilgan
        for (final (c, d) in [(c1, 1.0), (c2, -1.0)]) {
          canvas.drawPath(
              Path()
                ..moveTo(c.dx - 16 * d, c.dy - 14)
                ..lineTo(c.dx + 12 * d, c.dy)
                ..lineTo(c.dx - 16 * d, c.dy + 14),
              ink(12));
        }
      default:
        _part(canvas, 'eye', e1);
        _part(canvas, 'eye_2', e2);
    }
    // qoshlar: chiranganda jahl (pastga), yig'lashda xafa (yuqoriga)
    if (s.eyes == _Eyes.cry || s.eyes == _Eyes.strain) {
      final kq = s.eyes == _Eyes.strain ? -1.0 : 1.0;
      for (final (c, d) in [(c1, 1.0), (c2, -1.0)]) {
        canvas.drawLine(Offset(c.dx - 26 * d, c.dy - 34 + 10 * kq),
            Offset(c.dx + 18 * d, c.dy - 38 - 6 * kq), ink(10));
      }
    }
    // ko'z yoshlari
    if (s.tears > 0) {
      final ph = (s.sec * 2.2) % 1;
      for (final (c, d) in [(c1, -1.0), (c2, 1.0)]) {
        canvas.drawPath(
            Path()
              ..moveTo(c.dx + 18 * d, c.dy + 8)
              ..quadraticBezierTo(
                  c.dx + 30 * d, c.dy + 50, c.dx + 24 * d, c.dy + 90),
            ink(12, const Color(0xF278C8FF)));
        canvas.drawOval(
            Rect.fromCenter(
                center: Offset(c.dx + 24 * d, c.dy + 90 + ph * 40),
                width: 18,
                height: 24),
            Paint()..color = const Color(0xF296D7FF));
      }
    }
    // ter tomchisi
    if (s.sweat > 0) {
      final ph = (s.sec * 1.6) % 1;
      final px = y < 0 ? 185.0 : 420.0, py = 50 + ph * 60;
      final drop = Path()
        ..moveTo(px, py - 28)
        ..quadraticBezierTo(px + 18, py, px, py + 12)
        ..quadraticBezierTo(px - 18, py, px, py - 28)
        ..close();
      final alpha = s.sweat * (1 - ph * .6);
      canvas.drawPath(
          drop, Paint()..color = const Color(0xFF8FD3FF).withValues(alpha: alpha));
      canvas.drawPath(drop,
          ink(6, const Color(0xFF3D8FC4).withValues(alpha: alpha)));
    }
    // yaqin qo'l (va qop) — tana oldida
    // qop qanot uchida osilib turadi
    if (s.bag == _BagMode.hand && carry != null) {
      final tip = carry == 'b' ? _tip(ar.shB, ar.b) : _tip(ar.shA, ar.a);
      _bag(canvas, _bagAt(s, tip), s.bw / _k, s.bh / _k, 1, 0, local: true);
    }
    if (y.abs() <= .35 || near == 'a' || carry == 'a') _arm(canvas, ar.shA, ar.a);
    if ((y.abs() <= .35 || near == 'b' || carry == 'b') && !gripB) {
      _arm(canvas, ar.shB, ar.b);
    }
    canvas.restore();
  }

  // ── QOP ────────────────────────────────────────────────────────

  void _bag(Canvas canvas, Offset c, double w, double h, double sq, double rot,
      {bool local = false}) {
    canvas.save();
    canvas.translate(c.dx, c.dy);
    canvas.rotate(rot);
    final ww = w * sq, hh = h * (1 + (1 - sq) * .35);
    final lw = local ? 10.0 : _clamp(h * .035, 3.2, 7);
    final top = -hh / 2, bot = hh / 2, neck = ww * .13;
    final body = Path()
      ..moveTo(-neck, top + hh * .16)
      ..cubicTo(-ww * .62, top + hh * .26, -ww * .6, bot - hh * .02, -ww * .32, bot)
      ..cubicTo(-ww * .12, bot + hh * .04, ww * .12, bot + hh * .04, ww * .32, bot)
      ..cubicTo(ww * .6, bot - hh * .02, ww * .62, top + hh * .26, neck, top + hh * .16)
      ..close();
    canvas.drawPath(
        body,
        Paint()
          ..shader = ui.Gradient.linear(Offset(-ww / 2, 0), Offset(ww / 2, 0),
              const [Color(0xFF4A4F58), Color(0xFF2C2F35)]));
    final edge = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = lw
      ..strokeJoin = StrokeJoin.round
      ..strokeCap = StrokeCap.round
      ..color = const Color(0xFF121317);
    canvas.drawPath(body, edge);
    final kt = top + hh * .16;
    final knot = Path()
      ..moveTo(-neck, kt)
      ..cubicTo(-ww * .3, top + hh * .04, -ww * .3, top - hh * .06, -ww * .17, top - hh * .05)
      ..cubicTo(-ww * .1, top - hh * .04, -ww * .04, top + hh * .06, 0, top + hh * .1)
      ..cubicTo(ww * .04, top + hh * .06, ww * .1, top - hh * .04, ww * .17, top - hh * .05)
      ..cubicTo(ww * .3, top - hh * .06, ww * .3, top + hh * .04, neck, kt)
      ..close();
    canvas.drawPath(knot, Paint()..color = const Color(0xFF3D4149));
    canvas.drawPath(knot, edge);
    canvas.drawOval(
        Rect.fromCenter(
            center: Offset(0, kt - hh * .01), width: ww * .2, height: hh * .07),
        Paint()..color = const Color(0xFF121317));
    final fold = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = lw * .65
      ..strokeCap = StrokeCap.round
      ..color = const Color(0xBF121317);
    canvas.drawPath(
        Path()
          ..moveTo(-ww * .05, top + hh * .28)
          ..quadraticBezierTo(-ww * .14, top + hh * .55, -ww * .08, top + hh * .78),
        fold);
    canvas.drawPath(
        Path()
          ..moveTo(ww * .2, top + hh * .3)
          ..quadraticBezierTo(ww * .28, top + hh * .55, ww * .2, top + hh * .74),
        fold);
    canvas.drawPath(
        Path()
          ..moveTo(-ww * .36, top + hh * .42)
          ..quadraticBezierTo(-ww * .4, top + hh * .6, -ww * .3, top + hh * .78),
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = lw * .8
          ..strokeCap = StrokeCap.round
          ..color = const Color(0x73FFFFFF));
    canvas.restore();
  }

  /// Sudralayotgan qop: tubi `bagPx` da yerda, `bagAng` burchakda egilgan.
  /// [pass] — eshikdan o'tayotganda ichki (devor orqasi) va tashqi qismi alohida.
  void _dragBag(Canvas canvas, _S s, _Pass pass) {
    if (s.bag != _BagMode.drag || s.bagPx.isNaN) return;
    final sq = s.bagSq, hh = s.bh * (1 + (1 - sq) * .35), ww = s.bw * sq;
    final py = _floor - (ww / 2) * math.sin(s.bagAng).abs() * .85 + 2;
    canvas.save();
    if (pass == _Pass.inside) {
      canvas.clipRect(const Rect.fromLTRB(0, 0, _dr + 1, _h));
    } else if (pass == _Pass.outside) {
      canvas.clipRect(const Rect.fromLTRB(_dr, 0, _w, _h));
    }
    canvas.translate(s.bagPx, py);
    canvas.rotate(s.bagAng);
    _bag(canvas, Offset(0, -hh / 2), s.bw, s.bh, sq, 0);
    canvas.restore();
  }

  void _flying(Canvas canvas, _S s, Offset a) {
    if (s.bag != _BagMode.fly) return;
    final u = _seg(s.sec, s.flyT0, s.flyT1);
    final x = _lerp(a.dx, _bc, u);
    final y = _lerp(a.dy, _bt + 6, u) - math.sin(u * math.pi) * s.flyPeak;
    canvas.save();
    // og'izga kirgach old gardish orqasida yo'qoladi
    if (u > .7) canvas.clipRect(const Rect.fromLTWH(0, 0, _w, _bt + 2));
    final st = 1 + .12 * math.sin(u * math.pi);
    // ozgina aylanib uchadi
    _bag(canvas, Offset(x, y), s.bw / st, s.bh * st, 1,
        u * .9 + math.sin(u * math.pi) * .35);
    canvas.restore();
  }

  // ── UY, PANJARA, QUTI ──────────────────────────────────────────

  void _ground(Canvas canvas) {
    canvas.drawRect(const Rect.fromLTWH(0, _floor, _w, _h - _floor),
        Paint()..color = const Color(0xFF1F1A16));
    canvas.drawRect(const Rect.fromLTWH(0, _floor, _w, 8),
        Paint()..color = const Color(0xFF2B241E));
  }

  void _fence(Canvas canvas) {
    const top = _floor - 150, step = 74.0;
    const ink = Color(0xFF4A2A12);
    for (final yy in const [top + 40, top + 108]) {
      final r = Rect.fromLTWH(_wallR, yy, _w - _wallR, 18);
      canvas.drawRect(r, Paint()..color = const Color(0xFF7A4A26));
      canvas.drawRect(
          r,
          Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = 4
            ..color = ink);
    }
    for (var x = _wallR + 20; x < _w + step; x += step) {
      final p = Path()
        ..moveTo(x, _floor)
        ..lineTo(x, top + 14)
        ..lineTo(x + 20, top)
        ..lineTo(x + 40, top + 14)
        ..lineTo(x + 40, _floor)
        ..close();
      canvas.drawPath(
          p,
          Paint()
            ..shader = ui.Gradient.linear(Offset(x, 0), Offset(x + 40, 0),
                const [Color(0xFF9C6436), Color(0xFF83512A)]));
      canvas.drawPath(
          p,
          Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = 5
            ..strokeJoin = StrokeJoin.round
            ..color = ink);
      canvas.drawLine(
          Offset(x + 9, top + 24),
          Offset(x + 9, _floor - 14),
          Paint()
            ..strokeWidth = 4
            ..color = const Color(0x38FFDCAA));
      canvas.drawCircle(Offset(x + 20, top + 49), 3.5, Paint()..color = ink);
      canvas.drawCircle(Offset(x + 20, top + 117), 3.5, Paint()..color = ink);
    }
    // panjara orqa fonda — biroz xira
    canvas.drawRect(const Rect.fromLTWH(_wallR, top - 2, _w - _wallR, _floor - top + 2),
        Paint()..color = const Color(0x47141414));
  }

  void _houseBack(Canvas canvas, _S s) {
    const r = Rect.fromLTRB(_dl, _dt, _dr, _floor);
    canvas.drawRect(
        r,
        Paint()
          ..shader = ui.Gradient.linear(const Offset(0, _dt), const Offset(0, _floor),
              const [Color(0xFF1D140E), Color(0xFF3A271B)]));
    canvas.drawRect(r, Paint()..color = Color.fromRGBO(255, 200, 120, .25 * s.door));
  }

  void _houseFront(Canvas canvas) {
    // devor (eshik o'yig'i bilan)
    final wall = Path()
      ..fillType = PathFillType.evenOdd
      ..moveTo(-10, _floor)
      ..lineTo(-10, 170)
      ..lineTo(_wallR, 262)
      ..lineTo(_wallR, _floor)
      ..close()
      ..addRect(const Rect.fromLTRB(_dl, _dt, _dr, _floor));
    canvas.drawPath(
        wall,
        Paint()
          ..shader = ui.Gradient.linear(Offset.zero, const Offset(_wallR, 0),
              const [Color(0xFFE9C79A), Color(0xFFD4AB7A)]));
    canvas.save();
    canvas.clipPath(wall);
    final plank = Paint()
      ..strokeWidth = 4
      ..color = const Color(0x59966440);
    for (var yy = 200.0; yy < _floor; yy += 34) {
      canvas.drawLine(Offset(-10, yy), Offset(_wallR, yy), plank);
    }
    canvas.restore();
    canvas.drawLine(
        const Offset(_wallR, 262),
        const Offset(_wallR, _floor),
        Paint()
          ..strokeWidth = 8
          ..color = const Color(0xFF7A4A26));
    // poydevor
    const base = Rect.fromLTWH(-10, _floor - 22, _wallR + 10, 22);
    canvas.drawRect(base, Paint()..color = const Color(0xFF8C8A86));
    canvas.drawRect(
        base,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 5
          ..color = const Color(0xFF5A5855));
    // eshik ramkasi
    final frameFill = Paint()..color = const Color(0xFFAA5A1F);
    final frameInk = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 7
      ..color = const Color(0xFF6D3605);
    for (final r in const [
      Rect.fromLTWH(_dl - 14, _dt - 14, 14, _floor - _dt + 14),
      Rect.fromLTWH(_dr, _dt - 14, 14, _floor - _dt + 14),
      Rect.fromLTWH(_dl - 14, _dt - 14, _dr - _dl + 28, 14),
    ]) {
      canvas.drawRect(r, frameFill);
      canvas.drawRect(r, frameInk);
    }
    // tom: chap tepadan devor ustiga (cherepitsa)
    final roofEdge = Path()
      ..moveTo(-20, 110)
      ..lineTo(_wallR + 46, 262)
      ..lineTo(_wallR + 46, 286)
      ..lineTo(-20, 140)
      ..close();
    canvas.drawPath(roofEdge, Paint()..color = const Color(0xFFC4463A));
    final roofInk = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 7
      ..strokeJoin = StrokeJoin.round
      ..color = const Color(0xFF7A2620);
    canvas.drawPath(roofEdge, roofInk);
    final tiles = Path()
      ..moveTo(-20, 30)
      ..lineTo(_wallR + 46, 262)
      ..lineTo(-20, 262)
      ..close();
    canvas.drawPath(tiles, Paint()..color = const Color(0xFFD3554A));
    canvas.save();
    canvas.clipPath(tiles);
    final row = Paint()
      ..strokeWidth = 5
      ..color = const Color(0xB37A2620);
    for (var i = 0; i < 8; i++) {
      final o = i * 34.0;
      canvas.drawLine(Offset(-20, 30 + o), Offset(_wallR + 46, 262 + o), row);
    }
    canvas.restore();
    canvas.drawLine(const Offset(-20, 30), const Offset(_wallR + 46, 262), roofInk);
  }

  /// Tabaqa chap ilmoqda ichkariga aylanadi (perspektiva).
  void _doorPanel(Canvas canvas, _S s) {
    final phi = s.door * 1.45;
    const x0 = _dl, y0 = _dt, y1 = _floor, pw = _dr - _dl;
    final depth = math.sin(phi) * .2;
    final xf = x0 + pw * math.cos(phi);
    const yc = (y0 + y1) / 2;
    final hh = (y1 - y0) / 2 * (1 - depth);
    if ((xf - x0).abs() < 2) return;
    canvas.save();
    canvas.clipRect(const Rect.fromLTWH(_dl, 0, _w, _h));
    final q = Path()
      ..moveTo(x0, y0)
      ..lineTo(xf, yc - hh)
      ..lineTo(xf, yc + hh)
      ..lineTo(x0, y1)
      ..close();
    final sh = math.min(1.0, s.door * 1.2);
    canvas.drawPath(
        q,
        Paint()
          ..shader = ui.Gradient.linear(const Offset(x0, 0), Offset(xf, 0), [
            Color.lerp(const Color(0xFFD98A45), const Color(0xFF8F4A18), sh)!,
            Color.lerp(const Color(0xFFC7773A), const Color(0xFF6F3810), sh)!,
          ]));
    Offset at(double u, double v) => Offset(_lerp(x0, xf, u),
        _lerp(_lerp(y0, yc - hh, u), _lerp(y1, yc + hh, u), v));
    final panelInk = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 5
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
    final kr = Rect.fromCenter(
        center: at(.86, .55),
        width: 2 * (8 * math.cos(phi).abs() + 2),
        height: 16);
    canvas.drawOval(kr, Paint()..color = const Color(0xFFFFD527));
    canvas.drawOval(
        kr,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 4
          ..color = const Color(0xFF6D3605));
    canvas.drawPath(
        q,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 7
          ..strokeJoin = StrokeJoin.round
          ..color = const Color(0xFF6D3605));
    canvas.restore();
  }

  static final Rect _mouth = Rect.fromCenter(
      center: const Offset(_bc, _bt), width: _br - _bl + 16, height: 26);

  void _binBack(Canvas canvas, _S s) {
    canvas.save();
    canvas.translate(_bc, _bt);
    canvas.rotate(s.binWob * .06);
    canvas.translate(-_bc, -_bt);
    canvas.drawOval(_mouth, Paint()..color = const Color(0xFF163F49));
    canvas.restore();
  }

  void _binFront(Canvas canvas, _S s) {
    canvas.save();
    canvas.translate(_bc, _floor);
    canvas.rotate(s.binWob * .06);
    canvas.translate(-_bc, -_floor);
    final body = Path()
      ..moveTo(_bl, _bt)
      ..lineTo(_br, _bt)
      ..lineTo(_br - 12, _floor - 14)
      ..quadraticBezierTo(_br - 14, _floor, _br - 30, _floor)
      ..lineTo(_bl + 30, _floor)
      ..quadraticBezierTo(_bl + 14, _floor, _bl + 12, _floor - 14)
      ..close();
    canvas.drawPath(
        body,
        Paint()
          ..shader = ui.Gradient.linear(const Offset(_bl, 0), const Offset(_br, 0),
              const [Color(0xFF58B3C9), Color(0xFF3F97AD), Color(0xFF2E7B8F)],
              const [0, .55, 1]));
    final ink = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 7
      ..strokeJoin = StrokeJoin.round
      ..strokeCap = StrokeCap.round
      ..color = const Color(0xFF1B4D59);
    canvas.drawPath(body, ink);
    final rib = Paint()
      ..strokeWidth = 5
      ..strokeCap = StrokeCap.round
      ..color = const Color(0xBF1B4D59);
    for (final f in const [.3, .5, .7]) {
      final x = _lerp(_bl + 18, _br - 18, f);
      canvas.drawLine(
          Offset(x, _bt + 26), Offset(_lerp(x, _bc, .08), _floor - 22), rib);
    }
    canvas.drawLine(
        const Offset(_bl + 18, _bt + 24),
        const Offset(_bl + 24, _bt + 84),
        Paint()
          ..strokeWidth = 6
          ..strokeCap = StrokeCap.round
          ..color = const Color(0x8CFFFFFF));
    // og'iz gardishi (doim ochiq)
    canvas.drawOval(_mouth, ink);
    canvas.restore();
  }

  // ── CHANG VA UCHQUNLAR ─────────────────────────────────────────

  void _puff(Canvas canvas, _S s) {
    if (!(s.puff > 0 && s.puff < 1)) return;
    final a = 1 - _eio(s.puff), k = _eout(s.puff);
    canvas.saveLayer(null, Paint()..color = Color.fromRGBO(0, 0, 0, .9 * a));
    final p = Paint()..color = const Color(0xFFC7CCC1);
    for (final d in const [
      [-48.0, -6.0, 13.0, 1.0],
      [-22.0, -26.0, 17.0, .8],
      [6.0, -30.0, 16.0, .9],
      [32.0, -20.0, 14.0, 1.0],
      [54.0, -4.0, 11.0, 1.1],
    ]) {
      canvas.drawCircle(
          Offset(_bc + d[0] * (1 + k * .7 * d[3]), _bt - 10 + d[1] * (.6 + k * .9)),
          d[2] * (.5 + k * .8) * (1 - .3 * s.puff),
          p);
    }
    canvas.restore();
  }

  void _sparks(Canvas canvas, _S s) {
    if (!(s.spark > 0 && s.spark < 1)) return;
    const pts = [
      [_bl - 14, _bt - 80, 1.0],
      [_br + 8, _bt - 60, .8],
      [_bc + 14, _bt - 115, 1.1],
    ];
    final paint = Paint()..color = const Color(0xFFFFD527);
    for (var i = 0; i < pts.length; i++) {
      final u = _clamp(s.spark * 1.4 - i * .18, 0, 1);
      if (u <= 0 || u >= 1) continue;
      final r = math.sin(u * math.pi) * 15 * pts[i][2];
      canvas.save();
      canvas.translate(pts[i][0], pts[i][1] - u * 22);
      canvas.rotate(u * 1.5);
      final star = Path();
      for (var j = 0; j < 8; j++) {
        final an = j * math.pi / 4;
        final rr = j.isOdd ? r * .35 : r;
        if (j == 0) {
          star.moveTo(math.cos(an) * rr, math.sin(an) * rr);
        } else {
          star.lineTo(math.cos(an) * rr, math.sin(an) * rr);
        }
      }
      star.close();
      canvas.drawPath(star, paint);
      canvas.restore();
    }
  }

  @override
  bool shouldRepaint(_ScenePainter old) =>
      old.anim != anim || old.bytes != bytes;
}
