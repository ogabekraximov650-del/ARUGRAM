// lib/widgets/trend_chart.dart — TREYDING CHIZIG'IDEK GRAFIK.
//
// TALAB (foydalanuvchi): "statistika tizimini qo'sh, huddi treyding
// chizig'idek" — ilova statistikasi va majburiy kanallar uchun.
//
// Ma'lumot: `GET /api/stats/series?metric=..&range=24h|7d|30d|all`
// (`worker/src/lib.rs` -> `stats_series`, Cloudflare keshida 5 daqiqa).
// Ilovada ham davr bo'yicha 5 daqiqa eslab qolinadi — oyna qayta
// ochilganda server qayta so'ralmaydi.
//
// Ko'rinish: silliq chiziq va ostida gradient, yuqorida tanlangan davr
// yig'indisi va o'zgarish foizi (davrning ikkinchi yarmi birinchisiga
// nisbatan, yashil ↑ / qizil ↓). Barmoq bilan bosib yoki surib —
// nuqtaning aniq qiymati va vaqti.

import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

import '../services/auth_service.dart';
import 'glass.dart';

class TrendChart extends StatefulWidget {
  /// `views`, `anime_views`, `season_views`, `watch_ms`, `traffic`,
  /// `users`, `chan` yoki `chan:<id>`.
  final String metric;

  /// Qiymatni yozuvga aylantirish (masalan baytlar -> "1.2 GB").
  final String Function(int) format;

  final Color color;
  final double height;

  const TrendChart({
    super.key,
    required this.metric,
    required this.format,
    this.color = AppColors.accent2,
    this.height = 150,
  });

  @override
  State<TrendChart> createState() => _TrendChartState();
}

class _Point {
  final String t;
  final int v;
  const _Point(this.t, this.v);
}

/// (metric|range) -> (vaqt, nuqtalar).
final Map<String, (DateTime, List<_Point>)> _memo = {};

class _TrendChartState extends State<TrendChart> {
  static const _ranges = [
    ('24h', '24 soat'),
    ('7d', '7 kun'),
    ('30d', '30 kun'),
    ('all', 'Hammasi'),
  ];

  String _range = '7d';
  List<_Point>? _points;
  bool _loading = false;
  String? _error;
  int? _touch;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final key = '${widget.metric}|$_range';
    final hit = _memo[key];
    if (hit != null && DateTime.now().difference(hit.$1).inMinutes < 5) {
      setState(() {
        _points = hit.$2;
        _error = null;
      });
      return;
    }
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final t = AuthService.instance.sessionToken;
      final r = await http.get(
        Uri.parse('$kApiBase/api/stats/series'
            '?metric=${Uri.encodeQueryComponent(widget.metric)}&range=$_range'),
        headers: {if (t != null) 'Authorization': 'Bearer $t'},
      ).timeout(const Duration(seconds: 20));
      if (!mounted) return;
      if (r.statusCode != 200) {
        setState(() {
          _loading = false;
          _error = 'Yuklanmadi (${r.statusCode})';
        });
        return;
      }
      final j = jsonDecode(r.body) as Map<String, dynamic>;
      final pts = (j['points'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map((p) => _Point('${p['t']}', (p['v'] as num?)?.toInt() ?? 0))
          .toList();
      _memo[key] = (DateTime.now(), pts);
      setState(() {
        _points = pts;
        _loading = false;
        _touch = null;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = 'Internet yo\'q';
      });
    }
  }

  /// "2026-10-04T13" -> "4-okt 13:00", "2026-10-04" -> "4-okt 2026".
  static String _label(String t) {
    const months = [
      'yan', 'fev', 'mar', 'apr', 'may', 'iyun',
      'iyul', 'avg', 'sen', 'okt', 'noy', 'dek',
    ];
    final d = t.split('T');
    final p = d.first.split('-');
    if (p.length < 3) return t;
    final m = int.tryParse(p[1]) ?? 1;
    final day = int.tryParse(p[2]) ?? 1;
    final base = '$day-${months[(m - 1).clamp(0, 11)]}';
    return d.length > 1 ? '$base ${d[1]}:00' : '$base ${p[0]}';
  }

  @override
  Widget build(BuildContext context) {
    final pts = _points ?? const <_Point>[];
    final total = pts.fold<int>(0, (a, p) => a + p.v);
    final half = pts.length ~/ 2;
    final first = pts.take(half).fold<int>(0, (a, p) => a + p.v);
    final second = pts.skip(pts.length - half).fold<int>(0, (a, p) => a + p.v);
    final change = first > 0 ? (second - first) * 100 / first : null;
    final up = (change ?? 0) >= 0;
    final touched = _touch != null && _touch! < pts.length ? pts[_touch!] : null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    touched != null
                        ? widget.format(touched.v)
                        : widget.format(total),
                    style: const TextStyle(
                        color: Colors.white,
                        fontSize: 20,
                        fontWeight: FontWeight.w800),
                  ),
                  Text(
                    touched != null
                        ? _label(touched.t)
                        : 'Tanlangan davr bo\'yicha jami',
                    style: const TextStyle(color: Colors.white54, fontSize: 11.5),
                  ),
                ],
              ),
            ),
            if (change != null && touched == null)
              Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                decoration: BoxDecoration(
                  color: (up ? AppColors.success : AppColors.danger)
                      .withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Text(
                  '${up ? '▲' : '▼'} ${change.abs().toStringAsFixed(1)}%',
                  style: TextStyle(
                    color: up ? AppColors.success : AppColors.danger,
                    fontWeight: FontWeight.w800,
                    fontSize: 12.5,
                  ),
                ),
              ),
          ],
        ),
        const SizedBox(height: 10),
        SizedBox(
          height: widget.height,
          child: _loading && _points == null
              ? const Center(
                  child: SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(
                        strokeWidth: 2, color: Colors.white38),
                  ),
                )
              : _error != null && _points == null
                  ? Center(
                      child: Text(_error!,
                          style: const TextStyle(
                              color: Colors.white54, fontSize: 12.5)))
                  : LayoutBuilder(
                      builder: (context, box) {
                        int? at(double x) => pts.length < 2
                            ? null
                            : (x / box.maxWidth * (pts.length - 1))
                                .round()
                                .clamp(0, pts.length - 1);
                        return GestureDetector(
                          behavior: HitTestBehavior.opaque,
                          onTapDown: (d) =>
                              setState(() => _touch = at(d.localPosition.dx)),
                          onHorizontalDragUpdate: (d) =>
                              setState(() => _touch = at(d.localPosition.dx)),
                          onHorizontalDragEnd: (_) =>
                              setState(() => _touch = null),
                          onTapUp: (_) => setState(() => _touch = null),
                          child: CustomPaint(
                            size: Size(box.maxWidth, widget.height),
                            painter: _TrendPainter(
                              values: [for (final p in pts) p.v.toDouble()],
                              color: widget.color,
                              touch: _touch,
                            ),
                          ),
                        );
                      },
                    ),
        ),
        if (pts.length >= 2) ...[
          const SizedBox(height: 4),
          Row(
            children: [
              Text(_label(pts.first.t),
                  style: const TextStyle(color: Colors.white38, fontSize: 10)),
              const Spacer(),
              Text(_label(pts.last.t),
                  style: const TextStyle(color: Colors.white38, fontSize: 10)),
            ],
          ),
        ],
        const SizedBox(height: 10),
        Row(
          children: [
            for (final (k, label) in _ranges)
              Expanded(
                child: GestureDetector(
                  onTap: () {
                    if (_range == k) return;
                    setState(() => _range = k);
                    _load();
                  },
                  child: Container(
                    margin: const EdgeInsets.symmetric(horizontal: 3),
                    padding: const EdgeInsets.symmetric(vertical: 7),
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: _range == k
                          ? widget.color.withValues(alpha: 0.18)
                          : Colors.white.withValues(alpha: 0.05),
                      borderRadius: BorderRadius.circular(9),
                      border: Border.all(
                          color: _range == k
                              ? widget.color.withValues(alpha: 0.6)
                              : Colors.transparent),
                    ),
                    child: Text(label,
                        style: TextStyle(
                            color: _range == k ? Colors.white : Colors.white60,
                            fontSize: 12,
                            fontWeight: FontWeight.w600)),
                  ),
                ),
              ),
          ],
        ),
      ],
    );
  }
}

class _TrendPainter extends CustomPainter {
  final List<double> values;
  final Color color;
  final int? touch;
  _TrendPainter({required this.values, required this.color, this.touch});

  @override
  void paint(Canvas canvas, Size size) {
    // Yordamchi gorizontal chiziqlar.
    final grid = Paint()
      ..color = Colors.white.withValues(alpha: 0.06)
      ..strokeWidth = 1;
    for (var i = 1; i <= 3; i++) {
      final y = size.height * i / 4;
      canvas.drawLine(Offset(0, y), Offset(size.width, y), grid);
    }
    if (values.length < 2) return;
    final maxV = values.reduce(math.max);
    final top = maxV <= 0 ? 1.0 : maxV * 1.12;
    Offset pt(int i) => Offset(
          size.width * i / (values.length - 1),
          size.height - (values[i] / top) * (size.height - 6) - 3,
        );

    // Silliq chiziq (o'rta nuqtalar orqali kvadratik egri).
    final line = Path()..moveTo(pt(0).dx, pt(0).dy);
    for (var i = 1; i < values.length; i++) {
      final a = pt(i - 1), b = pt(i);
      final mid = Offset((a.dx + b.dx) / 2, (a.dy + b.dy) / 2);
      line.quadraticBezierTo(a.dx, a.dy, mid.dx, mid.dy);
      if (i == values.length - 1) line.lineTo(b.dx, b.dy);
    }
    final fill = Path.from(line)
      ..lineTo(size.width, size.height)
      ..lineTo(0, size.height)
      ..close();
    canvas.drawPath(
      fill,
      Paint()
        ..shader = LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [color.withValues(alpha: 0.35), color.withValues(alpha: 0.0)],
        ).createShader(Offset.zero & size),
    );
    canvas.drawPath(
      line,
      Paint()
        ..color = color
        ..strokeWidth = 2.2
        ..style = PaintingStyle.stroke
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round,
    );
    // Oxirgi nuqta.
    final last = pt(values.length - 1);
    canvas.drawCircle(last, 3.5, Paint()..color = color);

    // Tanlangan nuqta: vertikal chiziq va doira.
    final t = touch;
    if (t != null && t >= 0 && t < values.length) {
      final p = pt(t);
      canvas.drawLine(
        Offset(p.dx, 0),
        Offset(p.dx, size.height),
        Paint()
          ..color = Colors.white.withValues(alpha: 0.35)
          ..strokeWidth = 1,
      );
      canvas.drawCircle(p, 5, Paint()..color = Colors.white);
      canvas.drawCircle(p, 3.5, Paint()..color = color);
    }
  }

  @override
  bool shouldRepaint(_TrendPainter old) =>
      old.values != values || old.touch != touch || old.color != color;
}
