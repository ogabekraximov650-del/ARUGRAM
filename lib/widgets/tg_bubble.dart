// lib/widgets/tg_bubble.dart — TELEGRAM'DAGIDEK XABAR PUFAKCHASI.
//
// Manba: Cherrygram `ActionBar/MessageDrawable.java` (`generatePath`):
//
//   * burchak radiusi 17 dp (`SharedConfig.bubbleRadius`), guruh
//     ichidagi qo'shni burchak 6 dp (`nearRad`);
//   * guruhning OXIRGI xabarida pastki burchakda "dum": pufak dum
//     tomonidan 8 dp ichkarida, dum esa 6 dp radiusli yoy bilan
//     chetga (2.6 dp gacha) chiqadi;
//   * dumsiz (guruh o'rtasi) xabar ham o'sha 8 dp chiziqdan boshlanadi —
//     ketma-ket xabarlar bir tekisda turadi;
//   * rasm/video xabari (`TYPE_MEDIA`) — dumsiz, to'liq yumaloq.

import 'package:flutter/material.dart';

/// Telegram pufakchasining shakli.
@immutable
class TgBubbleShape {
  /// O'z xabari (o'ngda, dum o'ngda).
  final bool out;

  /// Dum chiziladimi (guruhning oxirgi xabari).
  final bool tail;

  /// Tepada/pastda shu yuboruvchining boshqa xabari bor — o'sha
  /// tomondagi burchak 6 dp.
  final bool topNear;
  final bool bottomNear;

  /// `TYPE_MEDIA` — dumsiz, 8 dp chekinishsiz.
  final bool media;

  const TgBubbleShape({
    required this.out,
    this.tail = true,
    this.topNear = false,
    this.bottomNear = false,
    this.media = false,
  });

  static const double radius = 17;
  static const double nearRadius = 6;

  /// Dum tomonidagi chekinish (`dp(8)`).
  static const double tailInset = 8;

  Path pathFor(Rect b) {
    final path = Path();
    var rad = radius;
    if (rad > b.height / 2) rad = b.height / 2;
    final near = nearRadius.clamp(0.0, rad);
    const small = 6.0;
    final drawTail = tail && !media;

    Rect r(double l, double t, double rr, double bb) =>
        Rect.fromLTRB(l, t, rr, bb);
    double deg(double d) => d * 3.141592653589793 / 180;

    if (out) {
      final right = media ? b.right : b.right - tailInset;
      final bottomRad = bottomNear ? near : rad;
      final topRightRad = topNear ? near : rad;
      // Pastki chiziq (o'ngdan chapga).
      if (drawTail) {
        path.moveTo(b.right - 2.6, b.bottom);
      } else {
        path.moveTo(right - bottomRad, b.bottom);
      }
      path.lineTo(b.left + rad, b.bottom);
      path.arcTo(r(b.left, b.bottom - rad * 2, b.left + rad * 2, b.bottom),
          deg(90), deg(90), false);
      // Chap chiziq, chap-tepa burchak.
      path.lineTo(b.left, b.top + rad);
      path.arcTo(r(b.left, b.top, b.left + rad * 2, b.top + rad * 2),
          deg(180), deg(90), false);
      // Tepa chiziq, o'ng-tepa burchak.
      path.lineTo(right - topRightRad, b.top);
      path.arcTo(
          r(right - topRightRad * 2, b.top, right, b.top + topRightRad * 2),
          deg(270), deg(90), false);
      // O'ng chiziq va dum (yoki burchak).
      if (drawTail) {
        path.lineTo(right, b.bottom - small - 3);
        path.arcTo(
            r(right, b.bottom - small * 2 - 9, b.right - 7 + small * 2,
                b.bottom - 1),
            deg(180),
            deg(-83),
            false);
      } else {
        path.lineTo(right, b.bottom - bottomRad);
        path.arcTo(r(right - bottomRad * 2, b.bottom - bottomRad * 2, right,
            b.bottom), deg(0), deg(90), false);
      }
      path.close();
    } else {
      final left = media ? b.left : b.left + tailInset;
      final bottomRad = bottomNear ? near : rad;
      final topLeftRad = topNear ? near : rad;
      if (drawTail) {
        path.moveTo(b.left + 2.6, b.bottom);
      } else {
        path.moveTo(left + bottomRad, b.bottom);
      }
      path.lineTo(b.right - rad, b.bottom);
      path.arcTo(r(b.right - rad * 2, b.bottom - rad * 2, b.right, b.bottom),
          deg(90), deg(-90), false);
      path.lineTo(b.right, b.top + rad);
      path.arcTo(r(b.right - rad * 2, b.top, b.right, b.top + rad * 2),
          deg(0), deg(-90), false);
      path.lineTo(left + topLeftRad, b.top);
      path.arcTo(
          r(left, b.top, left + topLeftRad * 2, b.top + topLeftRad * 2),
          deg(270), deg(-90), false);
      if (drawTail) {
        path.lineTo(left, b.bottom - small - 3);
        path.arcTo(
            r(b.left + 7 - small * 2, b.bottom - small * 2 - 9, left,
                b.bottom - 1),
            deg(0),
            deg(83),
            false);
      } else {
        path.lineTo(left, b.bottom - bottomRad);
        path.arcTo(r(left, b.bottom - bottomRad * 2, left + bottomRad * 2,
            b.bottom), deg(180), deg(-90), false);
      }
      path.close();
    }
    return path;
  }
}

/// Pufakcha: fon (dum bilan) va ichidagi kontent. Kontent dum
/// tomonidan [TgBubbleShape.tailInset] ichkarida turadi.
class TgBubble extends StatelessWidget {
  final TgBubbleShape shape;
  final Color color;
  final EdgeInsets padding;
  final Widget child;

  const TgBubble({
    super.key,
    required this.shape,
    required this.color,
    required this.child,
    this.padding = EdgeInsets.zero,
  });

  @override
  Widget build(BuildContext context) {
    final inset = shape.media ? 0.0 : TgBubbleShape.tailInset;
    return CustomPaint(
      painter: _BubblePainter(shape, color),
      child: Padding(
        padding: padding.add(EdgeInsets.only(
            left: shape.out ? 0 : inset, right: shape.out ? inset : 0)),
        child: child,
      ),
    );
  }
}

class _BubblePainter extends CustomPainter {
  final TgBubbleShape shape;
  final Color color;

  _BubblePainter(this.shape, this.color);

  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawPath(
        shape.pathFor(Offset.zero & size), Paint()..color = color);
  }

  @override
  bool shouldRepaint(_BubblePainter old) =>
      old.color != color ||
      old.shape.out != shape.out ||
      old.shape.tail != shape.tail ||
      old.shape.topNear != shape.topNear ||
      old.shape.bottomNear != shape.bottomNear ||
      old.shape.media != shape.media;
}

/// Rasm/video uchun: kontentni pufak shaklida kesadi (`TYPE_MEDIA`).
class TgBubbleClip extends CustomClipper<Path> {
  final TgBubbleShape shape;
  const TgBubbleClip(this.shape);

  @override
  Path getClip(Size size) => shape.pathFor(Offset.zero & size);

  @override
  bool shouldReclip(TgBubbleClip old) =>
      old.shape.out != shape.out ||
      old.shape.topNear != shape.topNear ||
      old.shape.bottomNear != shape.bottomNear;
}

/// Kun ajratgichi (`ChatActionCell`): markazda yarim shaffof
/// "tabletka"da sana.
class TgDateChip extends StatelessWidget {
  final String text;
  const TgDateChip(this.text, {super.key});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Center(
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
          decoration: BoxDecoration(
            color: const Color(0x66000000),
            borderRadius: BorderRadius.circular(14),
          ),
          child: Text(
            text,
            style: const TextStyle(
              color: Colors.white,
              fontSize: 14,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      ),
    );
  }
}

const _months = [
  'yanvar', 'fevral', 'mart', 'aprel', 'may', 'iyun',
  'iyul', 'avgust', 'sentabr', 'oktabr', 'noyabr', 'dekabr',
];

/// Ajratgich matni: "Bugun", "Kecha", "25-sentabr", "3-mart, 2025".
String tgDayLabel(int ms) {
  final d = DateTime.fromMillisecondsSinceEpoch(ms);
  final now = DateTime.now();
  final today = DateTime(now.year, now.month, now.day);
  final day = DateTime(d.year, d.month, d.day);
  final diff = today.difference(day).inDays;
  if (diff == 0) return 'Bugun';
  if (diff == 1) return 'Kecha';
  final s = '${d.day}-${_months[d.month - 1]}';
  return d.year == now.year ? s : '$s, ${d.year}';
}

/// Ikki vaqt bir kunmi.
bool tgSameDay(int a, int b) {
  final x = DateTime.fromMillisecondsSinceEpoch(a);
  final y = DateTime.fromMillisecondsSinceEpoch(b);
  return x.year == y.year && x.month == y.month && x.day == y.day;
}

/// Xabar vaqti — doim "HH:mm" (kun ajratgichda turadi).
String tgTime(int ms) {
  if (ms <= 0) return '';
  final d = DateTime.fromMillisecondsSinceEpoch(ms);
  String two(int n) => n.toString().padLeft(2, '0');
  return '${two(d.hour)}:${two(d.minute)}';
}
