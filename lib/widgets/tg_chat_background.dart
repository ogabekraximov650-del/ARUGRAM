// lib/widgets/tg_chat_background.dart — TELEGRAM'DAGIDEK CHAT FONI.
//
// TALAB (foydalanuvchi): "support chatning orqa foniga ozgina rang ber
// xuddi Telegram'nikidek".
//
// Telegram'ning standart foni (`res/raw/default_pattern.svg` + 4 rangli
// gradient, `MotionBackgroundDrawable`). Qorong'i mavzuda naqsh
// "manfiy" chiziladi: fon deyarli qora, naqsh chiziqlari esa gradient
// rangida xira yonib turadi (`wallpaper_neg_intensity`). Ranglar ilova
// aksentiga (to'q sariq) moslangan.
//
// Naqsh `assets/tg_pattern.png` — SVG'dan oq niqob qilib chizilgan
// (720x1480, ekran eniga cho'zilib, pastga takrorlanadi). Fon bir marta
// chiziladi (`RepaintBoundary`), surish paytida qayta chizilmaydi.

import 'dart:ui' as ui;

import 'package:flutter/material.dart';

class TgChatBackground extends StatefulWidget {
  final Widget child;
  const TgChatBackground({super.key, required this.child});

  @override
  State<TgChatBackground> createState() => _TgChatBackgroundState();
}

class _TgChatBackgroundState extends State<TgChatBackground> {
  static ui.Image? _pattern;
  ImageStream? _stream;
  ImageStreamListener? _listener;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_pattern != null || _stream != null) return;
    final s = const AssetImage('assets/tg_pattern.png')
        .resolve(createLocalImageConfiguration(context));
    _listener = ImageStreamListener((info, _) {
      _pattern ??= info.image.clone();
      info.dispose();
      if (mounted) setState(() {});
    }, onError: (_, __) {});
    s.addListener(_listener!);
    _stream = s;
  }

  @override
  void dispose() {
    final l = _listener;
    if (l != null) _stream?.removeListener(l);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      fit: StackFit.expand,
      children: [
        RepaintBoundary(
          child: CustomPaint(painter: _BgPainter(_pattern)),
        ),
        widget.child,
      ],
    );
  }
}

class _BgPainter extends CustomPainter {
  final ui.Image? pattern;
  _BgPainter(this.pattern);

  // Telegram 4 rangli gradient (burchaklar), aksentga moslangan.
  static const _c = [
    Color(0xFF6B3A1C), // chap-tepa
    Color(0xFF3E2A4A), // o'ng-tepa
    Color(0xFF2B3B4A), // chap-past
    Color(0xFF7A4A1E), // o'ng-past
  ];

  void _gradient(Canvas canvas, Rect r, Paint base) {
    // Ikki diagonal gradient ustma-ust — 4 burchak rangi.
    canvas.drawRect(
        r,
        Paint()
          ..blendMode = base.blendMode
          ..color = base.color
          ..shader = ui.Gradient.linear(
              r.topLeft, r.bottomRight, [_c[0], _c[3]]));
    canvas.drawRect(
        r,
        Paint()
          ..blendMode = base.blendMode == BlendMode.srcIn
              ? BlendMode.srcATop
              : base.blendMode
          ..shader = ui.Gradient.linear(r.topRight, r.bottomLeft,
              [_c[1].withValues(alpha: 0.6), _c[2].withValues(alpha: 0.6)]));
  }

  @override
  void paint(Canvas canvas, Size size) {
    final r = Offset.zero & size;
    // Deyarli qora asos.
    canvas.drawRect(r, Paint()..color = const Color(0xFF0C0A09));
    // Butun fonga juda xira rang (Telegram'da ham fon to'liq qora emas).
    canvas.saveLayer(r, Paint()..color = const Color(0x42000000));
    _gradient(canvas, r, Paint());
    canvas.restore();

    final p = pattern;
    if (p == null) return;
    // Naqsh: ekran eniga cho'zilib, pastga takrorlanadi; chiziqlari
    // gradient rangida.
    final scale = size.width / p.width;
    final tileH = p.height * scale;
    canvas.saveLayer(r, Paint()..color = const Color(0xA6000000));
    final src = Rect.fromLTWH(0, 0, p.width.toDouble(), p.height.toDouble());
    final paint = Paint()..filterQuality = FilterQuality.medium;
    for (var y = 0.0; y < size.height; y += tileH) {
      canvas.drawImageRect(
          p, src, Rect.fromLTWH(0, y, size.width, tileH), paint);
    }
    _gradient(canvas, r, Paint()..blendMode = BlendMode.srcIn);
    canvas.restore();
  }

  @override
  bool shouldRepaint(_BgPainter old) => !identical(old.pattern, pattern);
}
