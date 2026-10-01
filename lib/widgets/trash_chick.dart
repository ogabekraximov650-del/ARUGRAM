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
  // `preserve`: telefonda tizim animatsiyalari o'chiq bo'lsa ham Flutter
  // davomiylikni 20 barobar qisqartirmasin (jo'ja ko'z ilg'amas tez o'tardi).
  late final AnimationController _c = AnimationController(
      vsync: this,
      duration: TrashChickAnimation.durationFor(widget.bytes),
      animationBehavior: AnimationBehavior.preserve)
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

/// Old tomoni ko'rinadigan qop (eshik o'yig'ida; bo'g'zi jo'janing qo'lida).
class _End {
  final double cx, cy, d, sc, ry, tip;
  final bool inside;
  const _End(this.cx, this.cy, this.d, this.sc, this.ry, this.tip, this.inside);
}

// ── VAQT JADVALI ─────────────────────────────────────────────────
class _S {
  final int tier;
  final double sec, bh, bw;
  double door = 0, x = _dc, yaw = 0, ph = -1, stride = 1, lean = 0, lift = 0;
  double sx = 1, sy = 1, rot = 0, sweat = 0, shake = 0, tears = 0;
  double wind = 0, fling = 0, grip = 0, hop = 0, flail = 0, wipeBrow = 0;
  double puff = -1, spark = -1, binWob = 0, flyT0 = 0, flyT1 = 0, flyPeak = 0;
  bool pivotR = false, happy = false, dragging = false, stuck = false;
  bool holding = false, sob = false, rubEye = false, rubAlt = false;
  double flyBack = 0, jerk = 0, dragU = 0, dropAt = 99;

  /// Chuqurlik: 0 — devor tekisligi va tashqari, < 0 — uy ichida (uzoqroq).
  double depth = 0;
  String? throwArm; // 'a' / 'b' — otadigan qo'l (berilmasa — yaqini)
  _Eyes eyes = _Eyes.calm;
  _BagMode bag = _BagMode.hand;

  // eshik o'yig'idagi (old tomoni ko'rinadigan) qop
  bool hasEnd = false, endStuck = false;
  double endFrom = 0, endAt = 0, endOut = 0, endPop = 0;
  _End? endS;

  // yonboshlab yotgan, sudralayotgan qop: tubi (px) yerda, tuguni qo'lda
  double bagPx = double.nan, bagAng = math.pi / 2, bagSq = 1;
  bool side = false;
  Offset? knot; // ikki qo'l ushlab turgan nuqta (sahnada)

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
    // Eshikdan chiqmaydi: o'zining chap qo'li bilan ramkani orqasidan ushlab,
    // o'ngga-chapga mo'ralaydi va o'ng qo'li bilan qopni shu yerdan otadi.
    door = _eio(_seg(t, 0, .7)) - _eio(_seg(t, 6.1, 6.9));
    x = _dc + 18;
    throwArm = 'a';
    depth = -.3 - .4 * _eio(_seg(t, 5.8, 6.2)); // eshik o'yig'ida turadi
    var y = _lerp(0, 1, _eio(_seg(t, .85, 1.15)));
    y = _lerp(y, -1, _eio(_seg(t, 1.95, 2.35)));
    y = _lerp(y, 1, _eio(_seg(t, 2.85, 3.15)));
    y = _lerp(y, 0, _eio(_seg(t, 5.55, 5.85)));
    yaw = y;
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
    door = _eio(_seg(t, 0, .7)) - _eio(_seg(t, 12.7, 13.4));
    var y = _lerp(0, 1, _eio(_seg(t, .75, .95)));
    y = _lerp(y, 0, _eio(_seg(t, 7.0, 7.25)));
    y = _lerp(y, -1, _eio(_seg(t, 7.9, 8.2)));
    y = _lerp(y, -2, _eio(_seg(t, 12.35, 12.6))); // orqasini o'girib kiradi
    yaw = y;
    // ostonadan bizga tomon chiqadi, oxirida ichkariga kiradi
    depth = -.6 * (1 - _eio(_seg(t, .35, .85))) - .6 * _eio(_seg(t, 12.5, 12.95));
    if (t > .35 && t < .85) ph = _seg(t, .35, .85) * 2;
    var w = _walk(t, .85, 5.6, _dc, _stopX - 60, 58);
    x = w.x;
    if (t > .85) ph = w.ph;
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
    // Orqasi bilan (bizga orqasini o'girib) ostonadan chiqadi; qop old tomoni
    // bilan eshik o'yig'ida ko'rinadi; jo'ja yon tomonga o'tib tortadi, qop
    // ag'darilib yonboshlaydi va yo'lak bo'ylab sudraladi.
    door = _eio(_seg(t, 0, .7)) - _eio(_seg(t, 15.8, 16.4));
    const xSide = _dr + 80;
    var y = -2.0;
    y = _lerp(y, -1, _eio(_seg(t, 1.3, 1.7)));
    y = _lerp(y, 0, _eio(_seg(t, 9.0, 9.35)));
    y = _lerp(y, -1, _eio(_seg(t, 10.6, 10.9)));
    y = _lerp(y, -2, _eio(_seg(t, 15.2, 15.45)));
    yaw = y;
    depth = -.7 * (1 - _eio(_seg(t, .5, 1.3))) - .7 * _eio(_seg(t, 15.35, 15.8));
    x = _dc;
    if (t > .5 && t < 1.3) ph = _seg(t, .5, 1.3) * 2;
    if (t >= 1.3) {
      final w = _walk(t, 1.3, 1.9, _dc, xSide, 40);
      x = w.x;
      ph = w.ph;
    }
    bag = _BagMode.drag;
    hasEnd = true;
    endFrom = .5;
    endAt = 1.9;
    endOut = 2.3;
    dragging = t >= 1.6 && t < 8.6;
    holding = t >= .5 && t < 8.6;
    if (t >= 2.3) {
      final w = _walk(t, 2.3, 8.6, xSide, _dragX, 34);
      x = w.x;
      ph = w.ph;
      stride = .6;
    }
    lean = dragging ? .16 + .03 * math.sin(t * 9) : 0.0;
    eyes = dragging ? _Eyes.strain : _Eyes.calm;
    sweat = dragging ? 1.0 : math.max(0.0, 1 - (t - 8.7) * 1.5);
    dropAt = 8.6;
    if (t > 8.7 && t < 10.4) {
      final u = _seg(t, 8.7, 10.4);
      sy = 1 - .06 * math.sin(u * math.pi);
      sx = 1 + .05 * math.sin(u * math.pi);
      wipeBrow = math.sin(u * math.pi);
    }
    if (t > 10.9) {
      final w = _walk(t, 10.9, 15.2, _dragX, _dc, 58);
      x = w.x;
      ph = w.ph;
      stride = 1;
    }
  }

  void _tier4(double t) {
    // 5 GB+: orqasi bilan ostonadan chiqadi, qop eshikka tiqiladi; jo'ja yon
    // tomonga o'tib, tovonlariga tayanib chiranib tortadi. Qop birdan bo'shaydi:
    // muvozanat yo'qoladi va jo'ja tovoni atrofida orqasiga ag'dariladi
    // (og'irlik kuchi bilan tezlashib), dumaloq orqasida bir-ikki chayqalib
    // yotib qoladi, yig'lab o'tiradi, keyin turib qopni sudrab boradi.
    door = _eio(_seg(t, 0, .8)) - _eio(_seg(t, 21.7, 22.4));
    bag = _BagMode.drag;
    pivotR = true; // og'ish doim orqa tovon atrofida (sakrashsiz)
    const xSide = _dr + 90;
    var y = -2.0;
    y = _lerp(y, -1, _eio(_seg(t, 1.5, 1.9)));
    y = _lerp(y, 0, _eio(_seg(t, 6.5, 6.9))); // o'tirgach yuzini bizga buradi
    y = _lerp(y, -1, _eio(_seg(t, 8.0, 8.3))); // qopga qaraydi
    y = _lerp(y, 0, _eio(_seg(t, 16.2, 16.5)));
    y = _lerp(y, -1, _eio(_seg(t, 17.4, 17.7)));
    y = _lerp(y, -2, _eio(_seg(t, 21.0, 21.3)));
    yaw = y;
    depth = -.7 * (1 - _eio(_seg(t, .6, 1.5))) - .7 * _eio(_seg(t, 21.2, 21.7));
    x = _dc;
    if (t > .6 && t < 1.5) ph = _seg(t, .6, 1.5) * 2;
    hasEnd = true;
    endStuck = true;
    endFrom = .6;
    endPop = 4.45;
    if (t < 4.45) {
      if (t >= 1.5) {
        final w = _walk(t, 1.5, 2.1, _dc, xSide, 40);
        x = w.x;
        ph = w.ph;
      }
      jerk = math.max(0.0, math.sin(_seg(t, 2.1, 4.45) * math.pi * 6)) *
          _seg(t, 2.1, 4.45);
      if (t >= 2.1) x = xSide + jerk * 10;
      lean = _lerp(.08, .3, _eio(_seg(t, 2.1, 2.5))) + .08 * jerk;
      shake = t > 2.1 ? 1.0 : 0.0;
      eyes = t > 2.1 ? _Eyes.strain : _Eyes.calm;
      sweat = t > 2.3 ? 1.0 : 0.0;
      stuck = t >= 2.1;
      holding = true;
      dragging = t >= 1.5;
      return;
    }
    // ag'darilish: teskari mayatnik — burchak tezlanib o'sadi (u²), dumaloq
    // orqasi ustida dumalagani uchun tayanch nuqtasi orqaga siljiydi
    const tFall = 4.45, tHit = 4.8, lie = 1.32;
    double r;
    if (t < tHit) {
      final u = _seg(t, tFall, tHit);
      r = .3 + (lie - .3) * u * u;
      flyBack = u * u;
    } else {
      // yerga urilish: so'nuvchi chayqalish (har zarba kichikroq)
      final u = t - tHit;
      r = lie - .3 * math.exp(-u * 5) * math.sin(u * 11).abs();
      flyBack = 1;
    }
    r = _lerp(r, .28, _eio(_seg(t, 6.0, 6.6))); // zo'rg'a o'tirib oladi
    r = _lerp(r, 0, _eio(_seg(t, 7.4, 7.9))); // turadi
    rot = r;
    if (t > tHit && t < tHit + .22) {
      final kq = math.sin(_seg(t, tHit, tHit + .22) * math.pi);
      sy *= 1 - .14 * kq;
      sx *= 1 + .1 * kq;
    }
    // qo'llar beixtiyor silkinadi, ko'zlar avval ochiq (qo'rqib), keyin yig'i
    eyes = t < tHit + .1 ? _Eyes.calm : _Eyes.cry;
    sob = t > 5.1 && t < 7.4;
    flail = t < 5.0 ? math.sin(_seg(t, tFall, 5.0) * math.pi) : 0.0;
    rubEye = (t > 6.9 && t < 7.5) || (t > 16.0 && t < 21.0);
    rubAlt = t < 7.5; // navbatma-navbat ikki qo'li bilan
    tears = t > 5.0 ? 1.0 : 0.0;
    x = xSide;
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
      final w3 = _walk(t, 17.7, 21.0, _dragX, _dc, 40);
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
  // orqa ko'rinish: yuz qismlari bosh sirti bo'ylab chetga o'tadi
  if (yaw < -1) {
    final base = _pose(name, -1);
    if (!_faceSwap.containsKey(name)) return base;
    return _xform(base, 0, 1, -(-1 - yaw) * 170, 0);
  }
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
    final s = _resolve(_S(bytes, anim.value * _dur));
    Offset? release;
    if (s.bag == _BagMode.fly || s.bag == _BagMode.bin) {
      release = _handWorld(_S(bytes, s.flyT0 - 0.0001));
    }
    // uy ichida (devor orqasida) chiziladiganlar
    final chickIn = s.depth < -.02;
    final e = s.endS;
    final overBin = s.x > _bl - _hw;

    _ground(canvas);
    _fence(canvas);
    _houseBack(canvas, s);
    _binBack(canvas, s);
    if (e != null && e.inside) _bagEnd(canvas, s, e);
    if (chickIn) {
      _neck(canvas, s);
      _chick(canvas, s);
    }
    _houseFront(canvas);
    _doorPanel(canvas, s);
    // tashqarida
    if (e != null && !e.inside) _bagEnd(canvas, s, e);
    _dragBag(canvas, s);
    if (!chickIn) _neck(canvas, s);
    if (!chickIn && !overBin) _chick(canvas, s);
    _tearDrops(canvas, s);
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

  // ── QOP HOLATI ─────────────────────────────────────────────────
  //
  // Katta qop eshikdan old tomoni bilan (bizga qarab) chiqadi: avval eshik
  // o'yig'ida yumaloq old tomoni ko'rinadi, bo'g'zi jo'janing qo'lida. Tashqariga
  // chiqqach ag'darilib yonboshlaydi va yo'lak bo'ylab TO'LIQ YOTGAN holda
  // sudraladi: tubi orqada, uchidagi tuguni jo'janing ikki qo'lida.
  static const double _hOff = _hw * .55;

  static double _bagLen(_S s) => .9 * s.bh * (1 + (1 - s.bagSq) * .35);

  static double _bagWW(_S s) => s.bw * s.bagSq;

  static double _knotY(_S s) => _floor - _bagWW(s) / 2 * .85 + 2;

  static void _setKnot(_S s, double knotX) {
    s.bagAng = math.pi / 2;
    s.bagPx = knotX - _bagLen(s);
    s.knot = Offset(knotX, _knotY(s));
  }

  static _End? _endState(_S s) {
    if (!s.hasEnd) return null;
    final t = s.sec, d = s.bw;
    double sc;
    var inside = true;
    if (s.endStuck) {
      if (t < s.endFrom) return null;
      sc = _lerp(.78, 1, _eio(_seg(t, s.endFrom, 2.1))) + .03 * s.jerk;
      if (t >= s.endPop) {
        inside = false;
        sc = _lerp(1, 1.05, _eout(_seg(t, s.endPop, s.endPop + .1)));
      }
    } else {
      if (t < s.endFrom || t >= s.endOut) return null;
      sc = _lerp(.75, 1, _eio(_seg(t, s.endFrom, s.endAt)));
      if (t >= s.endAt) {
        inside = false;
        sc = 1;
      }
    }
    // ag'darilish: old ko'rinishdan yonbosh ko'rinishga (0..1)
    final tip = s.endStuck
        ? _seg(t, s.endPop, s.endPop + .35)
        : (t >= s.endAt ? _seg(t, s.endAt, s.endOut) : 0.0);
    if (tip >= 1) return null;
    final ry = d / 2 * .92 * sc;
    return _End(_dc, _floor - ry + 2 - (inside ? (1 - sc) * 40 : 0), d, sc, ry,
        tip, inside);
  }

  static _S _resolve(_S s) {
    if (s.bag != _BagMode.drag) return s;
    final t = s.sec;
    final e = s.endS = _endState(s);
    s.side = false;
    s.knot = null;
    if (e != null && (s.holding || s.stuck)) s.knot = Offset(e.cx + 18, e.cy);
    if (s.tier == 3) {
      if (t >= s.endAt) {
        s.side = t >= s.endOut;
        _setKnot(s, (t < s.dropAt ? s.x : _dragX) - _hOff);
        if (t >= s.dropAt) s.knot = null;
      }
      return s;
    }
    final pop = s.endPop;
    if (t < pop) return s;
    s.side = t >= pop + .35;
    // qop tortilgan tomonga (o'ngga) otilib chiqadi va ishqalanish bilan
    // sekinlashib to'xtaydi; ag'darilib yonboshlaydi
    const restPx = 65.0;
    s.bagSq = _lerp(.86, 1, _backOut(_seg(t, pop, pop + .4), 2.5));
    s.bagAng = math.pi / 2;
    s.bagPx = _lerp(restPx - 70, restPx, _eout(_seg(t, pop, pop + .7)));
    const xSide = _dr + 90, xLand = xSide + 80;
    s.x = _lerp(xSide, xLand, s.flyBack);
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
    if (t > 17.7) s.x = _walk(t, 17.7, 21.0, _dragX, _dc, 40).x;
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
    if (s.sob) {
      // ho'ngrash: keskin nafas olish, sekin chiqarish
      final c = (s.sec * 1.7) % 1;
      final v = c < .18 ? c / .18 : math.pow(1 - (c - .18) / .82, 2).toDouble();
      sy *= 1 + .06 * v;
      sx *= 1 - .03 * v;
      if (s.pivotR) rot += .03 * v;
    }
    // chuqurlik: ichkarida (depth < 0) — kichikroq va tepada (uzoqroq)
    sx *= 1 + .1 * s.depth;
    sy *= 1 + .1 * s.depth;
    lift -= 14 * s.depth;
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
    final y = s.yaw, ys = math.max(-1.0, y);
    final kk = 1 - .5 * ys.abs(), off = 34 * ys;
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
    // sudrash / tortish: ikkala qo'l bilan qop bo'g'zini yoki tugunini ushlaydi
    final knot = s.knot;
    if ((s.dragging || s.stuck || s.holding) && knot != null) {
      final tr = s.shake * math.sin(s.sec * 50) * 5;
      final k = m.unmap(knot);
      final cap = s.endS != null ? _armL * 1.35 : _armL * 1.9;
      a = _capTo(shA, Offset(k.dx + 8, k.dy + 12 + tr), cap);
      b = _capTo(shB, Offset(k.dx - 8, k.dy - 12 - tr), cap);
    }
    if (s.flail > 0) {
      final w = math.sin(s.sec * 30) * 30;
      a = _lerp2(a, _polar(shA, -140 + w, _armL), s.flail);
      b = _lerp2(b, _polar(shB, -40 - w, _armL), s.flail);
    }
    // ko'zini artib yig'laydi (o'tirganda — navbatma-navbat ikki qo'li bilan)
    if (s.rubEye && !s.dragging) {
      final r = math.sin(s.sec * 16) * 10;
      final e1 = _bbox(_pose('eye', y)).center;
      final e2 = _bbox(_pose('eye_2', y)).center;
      final ha = _capTo(shA, Offset(e1.dx + r, e1.dy + 20), _armL * 1.7);
      final hb = _capTo(shB, Offset(e2.dx - r, e2.dy + 20), _armL * 1.7);
      if (s.rubAlt) {
        if ((s.sec / .7).floor().isOdd) {
          a = ha;
        } else {
          b = hb;
        }
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

  /// Ikki qo'l o'rtasi (sahnada) — qop bo'g'zi shu yerda.
  static Offset _pullWorld(_S s) {
    final m = _matrix(s);
    final ar = _arms(s, m);
    return m.map(_lerp2(ar.a, ar.b, .5));
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
            center: Offset(s.x + lie * _hw, _floor + 3 + 14 * s.depth),
            width: 2 * (_hw * 1.05 * (1 - lift / 250) + lie * _hw * .6),
            height: 18),
        Paint()..color = Color.fromRGBO(0, 0, 0, _clamp(.3 - lift / 300, 0, 1)));

    canvas.save();
    canvas.transform(m.storage);
    final y = s.yaw;
    final ar = _arms(s, m);
    final near = ar.face > 0 ? 'b' : 'a';
    final carry = _carry(s, ar);
    final gripB = s.grip > .5; // ramkani ushlagan qo'l alohida chiziladi
    // orqa ko'rinish: ikkala qo'l ham tana orqasida
    final backV = y < -1.3;
    if (backV) {
      _arm(canvas, ar.shA, ar.a);
      _arm(canvas, ar.shB, ar.b);
    } else if (y.abs() > .35) {
      // uzoqdagi qo'l — tana orqasida (qop ko'targan qo'l doim oldinda)
      final far = near == 'b' ? 'a' : 'b';
      if (!(far == 'b' && gripB) && far != carry) {
        if (far == 'a') {
          _arm(canvas, ar.shA, ar.a);
        } else {
          _arm(canvas, ar.shB, ar.b);
        }
      }
    }
    for (final n in const ['body', 'head_bl3', 'head', 'head_bl1', 'head_bl2']) {
      _part(canvas, n, _pose(n, y));
    }
    // yuz: orqaga o'girilganda bosh sirti bo'ylab chetga o'tib yashirinadi
    canvas.save();
    if (y < -1) {
      canvas.clipPath(_path(_pose('body', y))
        ..addPath(_path(_pose('head', y)), Offset.zero));
    }
    for (final n in const ['beak', 'beak_bl', 'mouth']) {
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
    // ko'z yoshlari: yuzdan oqadigan oqim (tomchilar keyin sahnada tushadi)
    if (s.tears > 0) {
      for (final (c, d) in [(c1, -1.0), (c2, 1.0)]) {
        canvas.drawPath(
            Path()
              ..moveTo(c.dx + 16 * d, c.dy + 8)
              ..quadraticBezierTo(
                  c.dx + 28 * d, c.dy + 40, c.dx + 24 * d, c.dy + 70),
            ink(11, const Color(0xE678C8FF)));
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
    canvas.restore();
    if (backV) {
      canvas.restore();
      return;
    }
    // qop qanot uchida osilib turadi (mayatnik: qadam bilan tebranadi,
    // otishga tayyorlanishda orqaga og'adi)
    if (s.bag == _BagMode.hand && carry != null) {
      final tip = carry == 'b' ? _tip(ar.shB, ar.b) : _tip(ar.shA, ar.a);
      final sw = (s.ph >= 0
              ? .16 * math.sin(s.ph * math.pi)
              : .05 * math.sin(s.sec * 3)) +
          s.wind * .5 * ar.face;
      canvas.save();
      canvas.translate(tip.dx, tip.dy);
      canvas.rotate(sw);
      _bag(canvas, Offset(0, .4 * s.bh / _k), s.bw / _k, s.bh / _k, 1, 0,
          local: true);
      canvas.restore();
    }
    // yaqin qo'l (va qop ko'targan qo'l) — tana oldida
    final side = y.abs() > .35;
    if (!side || near == 'a' || carry == 'a') _arm(canvas, ar.shA, ar.a);
    if ((!side || near == 'b' || carry == 'b') && !gripB) {
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

  /// Yonboshlab yotgan, sudralayotgan qop: tubi `bagPx` da yerda.
  void _dragBag(Canvas canvas, _S s) {
    if (s.bag != _BagMode.drag || s.bagPx.isNaN || !s.side) return;
    final sq = s.bagSq, hh = s.bh * (1 + (1 - sq) * .35), ww = s.bw * sq;
    final py = _floor - (ww / 2) * math.sin(s.bagAng).abs() * .85 + 2;
    canvas.save();
    canvas.translate(s.bagPx, py);
    canvas.rotate(s.bagAng);
    _bag(canvas, Offset(0, -hh / 2), s.bw, s.bh, sq, 0);
    canvas.restore();
  }

  /// Eshik o'yig'idagi qop: old (yig'ilgan og'zi) tomoni bizga qarab turadi.
  /// `tip` > 0 — tortilgan tomonga (o'ngga) burilib yonboshlaydi: tik o'q
  /// atrofida aylanish proyeksiyasi — yon tanasi `sin` bilan uzayadi, og'iz
  /// tomoni `cos` bilan torayib o'ng uchiga o'tadi.
  void _bagEnd(Canvas canvas, _S s, _End e) {
    var rx = e.d / 2 * e.sc, ry = e.ry, cx = e.cx, cy = e.cy;
    if (e.tip > 0 && !s.bagPx.isNaN) {
      final sq = s.bagSq, hh = s.bh * (1 + (1 - sq) * .35), ww = s.bw * sq;
      final u = _eout(e.tip), th = u * math.pi / 2;
      final sn = math.sin(th), cs = math.cos(th);
      final py = _floor - ww / 2 * .85 + 2;
      final c = _lerp(e.cx, s.bagPx + hh / 2, u);
      cy = _lerp(cy, py, u);
      ry = _lerp(ry, ww / 2 * .9, u);
      canvas.drawOval(
          Rect.fromCenter(
              center: Offset(c, _floor + 2),
              width: 2 * (hh / 2 * sn + rx * cs) * .95,
              height: 20),
          Paint()..color = const Color(0x4D000000));
      if (sn > .02) {
        canvas.save();
        canvas.translate(c, py);
        canvas.scale(sn, 1);
        canvas.translate(-hh / 2, 0);
        canvas.rotate(math.pi / 2);
        _bag(canvas, Offset(0, -hh / 2), s.bw, s.bh, sq, 0);
        canvas.restore();
      }
      rx *= cs;
      cx = c + hh / 2 * sn;
      if (rx < 2) return;
    } else {
      if (rx <= 0 || ry <= 0) return;
      canvas.drawOval(
          Rect.fromCenter(
              center: Offset(cx, _floor + 2), width: 2 * rx * .95, height: 20),
          Paint()..color = const Color(0x4D000000));
    }
    canvas.save();
    canvas.translate(cx, cy);
    final oval = Rect.fromCenter(center: Offset.zero, width: 2 * rx, height: 2 * ry);
    canvas.drawOval(
        oval,
        Paint()
          ..shader = ui.Gradient.radial(
              Offset.zero,
              rx,
              const [Color(0xFF555A64), Color(0xFF25282D)],
              null,
              TileMode.clamp,
              null,
              Offset(-rx * .3, -ry * .35),
              rx * .1));
    canvas.drawOval(
        oval,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 6
          ..color = const Color(0xFF121317));
    final wr = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round
      ..color = const Color(0xBF121317);
    // og'iz tomon yig'ilgan burmalar (notekis, har biri boshqa uzunlikda)
    const folds = [
      [.3, .88, .3],
      [1.05, .62, -.25],
      [1.7, .8, .4],
      [2.55, .7, -.3],
      [3.3, .9, .2],
      [4.0, .55, -.35],
      [4.75, .78, .3],
      [5.5, .66, -.2],
    ];
    for (final f in folds) {
      final a = f[0], l = f[1], bend = f[2];
      canvas.drawPath(
          Path()
            ..moveTo(math.cos(a) * rx * l, math.sin(a) * ry * l)
            ..quadraticBezierTo(math.cos(a + bend) * rx * l * .55,
                math.sin(a + bend) * ry * l * .55, math.cos(a) * rx * .12,
                math.sin(a) * ry * .12),
          wr);
    }
    canvas.drawArc(
        Rect.fromCenter(center: Offset.zero, width: 2 * rx * .78, height: 2 * ry * .78),
        math.pi * 1.08,
        math.pi * .3,
        false,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 6
          ..color = const Color(0x59FFFFFF));
    canvas.drawCircle(Offset.zero, ry * .13, Paint()..color = const Color(0xFF121317));
    canvas.restore();
  }

  /// Qop bo'g'zi: eshikdagi qopning yig'ilgan og'zi jo'janing qo'llarigacha
  /// cho'zilgan — qop yonida keng, tugun tomon toraygan, burmalari bilan.
  void _neck(Canvas canvas, _S s) {
    final e = s.endS;
    if (e == null || !(s.holding || s.stuck)) return;
    final h = _pullWorld(s);
    final a = Offset(e.cx, e.cy);
    // tortilganda bo'g'iz tarang, aks holda biroz osiladi
    final c = Offset((a.dx + h.dx) / 2, (a.dy + h.dy) / 2 + (s.stuck ? 4 : 14));
    Offset at(double u) => a * ((1 - u) * (1 - u)) + c * (2 * u * (1 - u)) + h * (u * u);
    final w0 = math.min(54.0, e.ry * .42), w1 = 14.0;
    final left = <Offset>[], right = <Offset>[];
    const n = 14;
    for (var i = 0; i <= n; i++) {
      final u = i / n;
      final p = at(u);
      final dv = at(math.min(1, u + .01)) - at(math.max(0, u - .01));
      final d = dv.distance == 0 ? 1.0 : dv.distance;
      final nv = Offset(-dv.dy / d, dv.dx / d);
      // yig'ilgan og'iz: qopdan chiqishda keng, so'ng tez torayadi
      final w = w1 + (w0 - w1) * math.pow(1 - u, 2.2).toDouble();
      left.add(p + nv * (w / 2));
      right.add(p - nv * (w / 2));
    }
    final body = Path()..moveTo(left.first.dx, left.first.dy);
    for (final p in left.skip(1)) {
      body.lineTo(p.dx, p.dy);
    }
    for (final p in right.reversed) {
      body.lineTo(p.dx, p.dy);
    }
    body.close();
    canvas.drawPath(body, Paint()..color = const Color(0xFF34383F));
    canvas.drawPath(
        body,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 5
          ..strokeJoin = StrokeJoin.round
          ..color = const Color(0xFF121317));
    // bo'ylama burmalar
    final fold = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3
      ..strokeCap = StrokeCap.round
      ..color = const Color(0xB3121317);
    for (final k in const [.3, .7]) {
      final f = Path();
      for (var i = 0; i <= 10; i++) {
        final q = Offset.lerp(left[i], right[i], k)!;
        if (i == 0) {
          f.moveTo(q.dx, q.dy);
        } else {
          f.lineTo(q.dx, q.dy);
        }
      }
      canvas.drawPath(f, fold);
    }
    // tugun — qo'llar orasida
    canvas.drawCircle(h, 11, Paint()..color = const Color(0xFF3D4149));
    canvas.drawCircle(
        h,
        11,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 4
          ..color = const Color(0xFF121317));
  }

  /// Ko'z yoshi tomchilari: erkin tushadi (g), yerga tegsa sachraydi.
  void _tearDrops(Canvas canvas, _S s) {
    if (s.tears <= 0) return;
    final m = _matrix(s);
    const g = 2400.0;
    for (final (n, d, off) in const [('eye', -1.0, 0.0), ('eye_2', 1.0, .31)]) {
      final c = _bbox(_pose(n, s.yaw)).center;
      final p = m.map(Offset(c.dx + 24 * d, c.dy + 72));
      for (var i = 0; i < 3; i++) {
        final age = (s.sec + off + i * .23) % .69;
        final yy = p.dy + .5 * g * age * age, xx = p.dx + d * 30 * age;
        if (yy < _floor) {
          canvas.drawOval(
              Rect.fromCenter(center: Offset(xx, yy), width: 7, height: 10),
              Paint()..color = const Color(0xF296D7FF));
        } else {
          final tg = math.sqrt(math.max(0.0, 2 * (_floor - p.dy) / g));
          final a = age - tg;
          if (a < .15) {
            final k = a / .15;
            canvas.drawArc(
                Rect.fromCenter(
                    center: Offset(p.dx + d * 30 * tg, _floor),
                    width: 2 * (4 + 10 * k),
                    height: 2 * (2 + 2 * k)),
                math.pi,
                math.pi,
                false,
                Paint()
                  ..style = PaintingStyle.stroke
                  ..strokeWidth = 2.5
                  ..color = Color.fromRGBO(150, 215, 255, _clamp(1 - k, 0, 1)));
          }
        }
      }
    }
  }

  void _flying(Canvas canvas, _S s, Offset a) {
    if (s.bag != _BagMode.fly) return;
    final u = _seg(s.sec, s.flyT0, s.flyT1);
    // parabola: gorizontal tezlik doimiy
    final x = _lerp(a.dx, _bc, u);
    final y = _lerp(a.dy, _bt + 6, u) - 4 * s.flyPeak * u * (1 - u);
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
    // ostona: yer bilan tekis beton plita (ustidan yuriladi)
    const sill = Rect.fromLTWH(_dl - 14, _floor - 3, _dr - _dl + 28, 10);
    canvas.drawRect(sill, Paint()..color = const Color(0xFF9A9792));
    canvas.drawRect(const Rect.fromLTWH(_dl - 14, _floor - 3, _dr - _dl + 28, 3),
        Paint()..color = const Color(0xFFB5B2AC));
    canvas.drawRect(
        sill,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 3
          ..color = const Color(0xFF5A5855));
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
    // beton poydevor — faqat devor ostida (eshik o'yig'ida yo'q)
    for (final base in const [
      Rect.fromLTRB(-10, _floor - 22, _dl - 14, _floor),
      Rect.fromLTRB(_dr + 14, _floor - 22, _wallR, _floor),
    ]) {
      canvas.drawRect(base, Paint()..color = const Color(0xFF8C8A86));
      canvas.drawRect(
          base,
          Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = 5
            ..color = const Color(0xFF5A5855));
    }
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

  /// Eshik TASHQARIGA (bizga tomon) ochiladi: ilmog'i chap ustunda. Uzoq cheti
  /// bizga yaqinlashgani uchun perspektivada kattalashadi; 90° dan keyin devor
  /// ustiga yotadi va ichki (orqa) yuzi ko'rinadi.
  void _doorPanel(Canvas canvas, _S s) {
    final phi = s.door * 1.95;
    const x0 = _dl, y0 = _dt, y1 = _floor, pw = _dr - _dl;
    final xf = x0 + pw * math.cos(phi);
    const yc = (y0 + y1) / 2;
    final hh = (y1 - y0) / 2 * (1 + math.sin(phi) * .12);
    final q = Path()
      ..moveTo(x0, y0)
      ..lineTo(xf, yc - hh)
      ..lineTo(xf, yc + hh)
      ..lineTo(x0, y1)
      ..close();
    final back = math.cos(phi) < 0;
    final lit = math.cos(phi).abs();
    final c0 = back
        ? Color.lerp(const Color(0xFF7A3F14), const Color(0xFFA8612B), lit)!
        : Color.lerp(const Color(0xFFA8612B), const Color(0xFFD98A45), lit)!;
    final c1 = back
        ? Color.lerp(const Color(0xFF6A3510), const Color(0xFF93531F), lit)!
        : Color.lerp(const Color(0xFF93531F), const Color(0xFFC7773A), lit)!;
    canvas.drawPath(
        q,
        Paint()
          ..shader = ui.Gradient.linear(
              const Offset(x0, 0), Offset(xf == x0 ? x0 + 1 : xf, 0), [c0, c1]));
    Offset at(double u, double v) => Offset(_lerp(x0, xf, u),
        _lerp(_lerp(y0, yc - hh, u), _lerp(y1, yc + hh, u), v));
    if ((xf - x0).abs() > 10) {
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
          center: at(.86, .55), width: 2 * (8 * lit + 2), height: 16);
      canvas.drawOval(kr, Paint()..color = const Color(0xFFFFD527));
      canvas.drawOval(
          kr,
          Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = 4
            ..color = const Color(0xFF6D3605));
    }
    canvas.drawPath(
        q,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 7
          ..strokeJoin = StrokeJoin.round
          ..color = const Color(0xFF6D3605));
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
