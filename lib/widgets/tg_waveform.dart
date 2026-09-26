// lib/widgets/tg_waveform.dart — OVOZLI XABARNING TO'LQIN SHAKLI.
//
// Manba: Cherrygram `Components/SeekBarWaveform.java`,
// `MediaController.getWaveform`:
//
//   * to'lqin — 100 ta qiymat, har biri 0..31 (5 bit);
//   * chiziqlar har 3 dp da, eni 2 dp, uchlari yumaloq; balandligi
//     markazdan ±(7 × qiymat / 31) dp;
//   * eshitilgan qismi to'liq rangda, qolgani xira.
//
// Telegram to'lqinni yozish paytida hisoblaydi va hujjat bilan birga
// yuboradi. Bu yerda ham shunday: yozish paytidagi ovoz balandliklari
// (`record` — `onAmplitudeChanged`) 100 taga keltiriladi va xabar
// matniga `[wf:...]` bo'lib qo'shiladi (32 belgili alifbo, 100 belgi).

import 'dart:math' as math;

import 'package:flutter/material.dart';

const _alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef';
const _samples = 100;

/// Ovoz balandliklari (0..1) -> `[wf:...]`.
String tgEncodeWaveform(List<double> levels) {
  if (levels.isEmpty) return '';
  final out = StringBuffer('[wf:');
  // Eng baland qiymat 31 bo'ladi (Telegram ham normallaydi).
  final peak = levels.reduce(math.max);
  final k = peak <= 0 ? 0.0 : 31 / peak;
  for (var i = 0; i < _samples; i++) {
    final a = (i * levels.length / _samples).floor();
    final b = math.max(a + 1, ((i + 1) * levels.length / _samples).floor());
    var m = 0.0;
    for (var j = a; j < b && j < levels.length; j++) {
      m = math.max(m, levels[j]);
    }
    out.write(_alphabet[(m * k).round().clamp(0, 31)]);
  }
  out.write(']');
  return out.toString();
}

/// Xabar matnidan to'lqin (bo'lmasa `null`).
List<int>? tgDecodeWaveform(String body) {
  if (!body.startsWith('[wf:') || !body.endsWith(']')) return null;
  final s = body.substring(4, body.length - 1);
  final out = <int>[];
  for (final ch in s.split('')) {
    final v = _alphabet.indexOf(ch);
    if (v < 0) return null;
    out.add(v);
  }
  return out.isEmpty ? null : out;
}

/// Matn faqat to'lqindan iboratmi (ko'rsatilmaydi).
bool tgIsWaveformBody(String body) => tgDecodeWaveform(body) != null;

/// To'lqin chizig'i (`SeekBarWaveform`). [progress] 0..1.
class TgWaveform extends StatelessWidget {
  final List<int>? wave;
  final double progress;
  final Color played;
  final Color rest;
  final ValueChanged<double>? onSeek;

  const TgWaveform({
    super.key,
    required this.wave,
    required this.progress,
    required this.played,
    required this.rest,
    this.onSeek,
  });

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(builder: (context, box) {
      void seek(Offset p) =>
          onSeek?.call((p.dx / box.maxWidth).clamp(0.0, 1.0));
      return GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTapDown: onSeek == null ? null : (d) => seek(d.localPosition),
        onHorizontalDragUpdate:
            onSeek == null ? null : (d) => seek(d.localPosition),
        child: CustomPaint(
          size: Size(box.maxWidth, 30),
          painter: _WavePainter(wave, progress, played, rest),
        ),
      );
    });
  }
}

class _WavePainter extends CustomPainter {
  final List<int>? wave;
  final double progress;
  final Color played;
  final Color rest;

  _WavePainter(this.wave, this.progress, this.played, this.rest);

  /// `calculateHeights`: namunalar chiziqlar soniga taqsimlanadi.
  List<double> _heights(int count) {
    final w = wave;
    if (w == null || w.isEmpty) return List.filled(count, 0);
    final out = List<double>.filled(count, 0);
    for (var i = 0; i < count; i++) {
      final v = w[(i * w.length / count).floor().clamp(0, w.length - 1)];
      out[i] = 7 * v / 31;
    }
    return out;
  }

  @override
  void paint(Canvas canvas, Size size) {
    final count = (size.width / 3).floor();
    if (count <= 0) return;
    final h = _heights(count);
    final cy = size.height / 2;
    final paint = Paint()
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round;
    final split = progress * count;
    for (var i = 0; i < count; i++) {
      final x = i * 3.0 + 1;
      paint.color = i < split ? played : rest;
      canvas.drawLine(Offset(x, cy - h[i]), Offset(x, cy + h[i]), paint);
    }
  }

  @override
  bool shouldRepaint(_WavePainter old) =>
      old.progress != progress ||
      old.wave != wave ||
      old.played != played ||
      old.rest != rest;
}
