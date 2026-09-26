// lib/screens/storage_screen.dart — "XOTIRADAN FOYDALANISH" OYNASI.
//
// TALAB (foydalanuvchi): "Xotira ustiga bosilganda huddi Telegram'
// nikidek tozalash oynasi ochilsin".
//
// Telegram Android'dagi `CacheControlActivity` ko'rinishi:
//   * tepada toifalar halqasi (har bo'lak — o'z rangi va foizi),
//     o'rtasida tanlanganlar hajmi;
//   * "Xotiradan foydalanish" sarlavhasi va izoh;
//   * belgilash doirachali toifalar ro'yxati (nomi, foizi, hajmi);
//     belgisi olingan toifa halqada xiralashadi;
//   * "Keshni tozalash <hajm>" tugmasi;
//   * pastda: fayllar serverda qolishi haqida izoh.
//
// Faqat qayta yuklab olinadigan kesh tozalanadi
// (`StorageUsageService.clearable`); hisob ma'lumotlari alohida,
// belgisiz ko'rsatiladi.

import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../services/format.dart';
import '../services/storage_usage.dart';
import '../widgets/glass.dart';

class StorageScreen extends StatefulWidget {
  const StorageScreen({super.key});

  @override
  State<StorageScreen> createState() => _StorageScreenState();
}

/// Toifa ranglari (Telegram'dagidek yorqin, bir-biridan ajraladigan).
const Map<String, Color> _colors = {
  'Videolar': Color(0xFF3E9DF0),
  'Posterlar': Color(0xFF4CC86A),
  'Stikerlar va emojilar': Color(0xFFEF8C3A),
  'GIF va chat fayllari': Color(0xFF8E6FE0),
  'Vaqtinchalik fayllar': Color(0xFFD9B12B),
};

Color _colorOf(String label) => _colors[label] ?? const Color(0xFF8A8A8E);

class _StorageScreenState extends State<StorageScreen> {
  final StorageUsageService _svc = StorageUsageService.instance;

  /// Belgisi OLINGAN toifalar (qolganlari tanlangan).
  final Set<String> _off = {};
  bool _clearing = false;

  @override
  void initState() {
    super.initState();
    _svc.refresh();
  }

  List<StorageSlice> get _cache => _svc.usage.slices
      .where((s) => StorageUsageService.clearable.contains(s.label))
      .toList();

  List<StorageSlice> get _data => _svc.usage.slices
      .where((s) => !StorageUsageService.clearable.contains(s.label))
      .toList();

  int get _selectedBytes => _cache
      .where((s) => !_off.contains(s.label))
      .fold(0, (a, s) => a + s.bytes);

  Future<void> _clear() async {
    final labels = _cache
        .where((s) => !_off.contains(s.label))
        .map((s) => s.label)
        .toSet();
    if (labels.isEmpty || _clearing) return;
    final ok = await showDialog<bool>(
      context: context,
      builder: (c) => AlertDialog(
        backgroundColor: AppColors.card,
        title: const Text('Keshni tozalash'),
        content: Text(
            '${formatBytes(_selectedBytes)} hajmdagi kesh o\'chiriladi. '
            'Fayllar kerak bo\'lganda qayta yuklab olinadi.'),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(c, false),
              child: const Text('Bekor qilish')),
          TextButton(
              onPressed: () => Navigator.pop(c, true),
              child: const Text('Tozalash',
                  style: TextStyle(color: AppColors.accent2))),
        ],
      ),
    );
    if (ok != true || !mounted) return;
    setState(() => _clearing = true);
    await _svc.clear(labels);
    if (!mounted) return;
    setState(() {
      _clearing = false;
      _off.clear();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.bg,
      appBar: AppBar(
        backgroundColor: AppColors.bg,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
      ),
      body: AnimatedBuilder(
        animation: _svc,
        builder: (context, _) {
          final cache = _cache;
          final total = cache.fold<int>(0, (a, s) => a + s.bytes);
          final all = _svc.usage.totalBytes;
          if (!_svc.measured) {
            return const Center(
                child: CircularProgressIndicator(color: AppColors.accent));
          }
          return ListView(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 32),
            children: [
              Center(
                child: SizedBox(
                  width: 240,
                  height: 240,
                  child: _Donut(
                    slices: cache,
                    off: _off,
                    total: total,
                    selected: _selectedBytes,
                  ),
                ),
              ),
              const SizedBox(height: 22),
              const Text(
                'Xotiradan foydalanish',
                textAlign: TextAlign.center,
                style: TextStyle(fontSize: 22, fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 8),
              Text(
                'Ilova qurilmada jami ${formatBytes(all)} joy egallagan, '
                'shundan ${formatBytes(total)} — kesh.',
                textAlign: TextAlign.center,
                style: const TextStyle(color: Colors.white54, fontSize: 14),
              ),
              const SizedBox(height: 22),
              if (cache.isNotEmpty)
                Container(
                  decoration: BoxDecoration(
                    color: AppColors.card,
                    borderRadius: BorderRadius.circular(18),
                  ),
                  padding: const EdgeInsets.only(bottom: 14),
                  child: Column(
                    children: [
                      for (var i = 0; i < cache.length; i++) ...[
                        _Row(
                          slice: cache[i],
                          share: total > 0 ? cache[i].bytes / total : 0,
                          checked: !_off.contains(cache[i].label),
                          onTap: () => setState(() {
                            final l = cache[i].label;
                            if (!_off.remove(l)) _off.add(l);
                          }),
                        ),
                        if (i < cache.length - 1)
                          const Divider(
                              height: 1, indent: 64, color: Colors.white10),
                      ],
                      const SizedBox(height: 10),
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 14),
                        child: SizedBox(
                          width: double.infinity,
                          height: 52,
                          child: FilledButton(
                            style: FilledButton.styleFrom(
                              backgroundColor: AppColors.accent2,
                              disabledBackgroundColor:
                                  AppColors.accent2.withValues(alpha: 0.35),
                              shape: RoundedRectangleBorder(
                                  borderRadius: BorderRadius.circular(26)),
                            ),
                            onPressed: _selectedBytes > 0 && !_clearing
                                ? _clear
                                : null,
                            child: _clearing
                                ? const SizedBox(
                                    width: 22,
                                    height: 22,
                                    child: CircularProgressIndicator(
                                        strokeWidth: 2.4, color: Colors.white),
                                  )
                                : Text.rich(
                                    TextSpan(children: [
                                      const TextSpan(
                                          text: 'Keshni tozalash  ',
                                          style: TextStyle(
                                              fontWeight: FontWeight.w700)),
                                      TextSpan(
                                          text: formatBytes(_selectedBytes),
                                          style: const TextStyle(
                                              color: Colors.white70)),
                                    ]),
                                    style: const TextStyle(
                                        fontSize: 16, color: Colors.white),
                                  ),
                          ),
                        ),
                      ),
                    ],
                  ),
                )
              else
                const Padding(
                  padding: EdgeInsets.all(12),
                  child: Text('Kesh bo\'sh',
                      textAlign: TextAlign.center,
                      style: TextStyle(color: Colors.white54)),
                ),
              const SizedBox(height: 12),
              const Padding(
                padding: EdgeInsets.symmetric(horizontal: 8),
                child: Text(
                  'Barcha videolar, stikerlar, emojilar va rasmlar serverda '
                  'qoladi va kerak bo\'lganda qayta yuklab olinadi.',
                  style: TextStyle(color: Colors.white38, fontSize: 13),
                ),
              ),
              if (_data.isNotEmpty) ...[
                const SizedBox(height: 26),
                const Padding(
                  padding: EdgeInsets.only(left: 8, bottom: 8),
                  child: Text('ILOVA MA\'LUMOTLARI',
                      style: TextStyle(
                          color: AppColors.accent2,
                          fontSize: 13,
                          fontWeight: FontWeight.w600)),
                ),
                Container(
                  decoration: BoxDecoration(
                    color: AppColors.card,
                    borderRadius: BorderRadius.circular(18),
                  ),
                  child: Column(
                    children: [
                      for (final s in _data)
                        ListTile(
                          dense: true,
                          title: Text(s.label,
                              style: const TextStyle(fontSize: 15)),
                          trailing: Text(formatBytes(s.bytes),
                              style: const TextStyle(
                                  color: Colors.white54, fontSize: 14)),
                        ),
                    ],
                  ),
                ),
                const Padding(
                  padding: EdgeInsets.fromLTRB(8, 8, 8, 0),
                  child: Text(
                    'Tarix, sevimlilar va sozlamalar kesh emas — ular '
                    'tozalanmaydi.',
                    style: TextStyle(color: Colors.white38, fontSize: 13),
                  ),
                ),
              ],
            ],
          );
        },
      ),
    );
  }
}

class _Row extends StatelessWidget {
  final StorageSlice slice;
  final double share;
  final bool checked;
  final VoidCallback onTap;

  const _Row({
    required this.slice,
    required this.share,
    required this.checked,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final c = _colorOf(slice.label);
    return InkWell(
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 16),
        child: Row(
          children: [
            AnimatedContainer(
              duration: const Duration(milliseconds: 180),
              width: 26,
              height: 26,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: checked ? c : Colors.transparent,
                border: Border.all(
                    color: checked ? c : Colors.white30, width: 2),
              ),
              child: checked
                  ? const Icon(Icons.check_rounded,
                      size: 18, color: Colors.white)
                  : null,
            ),
            const SizedBox(width: 20),
            Expanded(
              child: Text.rich(
                TextSpan(children: [
                  TextSpan(text: slice.label),
                  TextSpan(
                    text: '  ${_pct(share)}',
                    style: const TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w700,
                        color: Colors.white70),
                  ),
                ]),
                style: const TextStyle(fontSize: 16),
              ),
            ),
            Text(formatBytes(slice.bytes),
                style: const TextStyle(
                    color: AppColors.accent2, fontSize: 16)),
          ],
        ),
      ),
    );
  }
}

String _pct(double v) {
  if (v <= 0) return '0%';
  if (v < 0.01) return '<1%';
  return '${(v * 100).round()}%';
}

/// Toifalar halqasi (Telegram'dagi `CacheChart`).
class _Donut extends StatelessWidget {
  final List<StorageSlice> slices;
  final Set<String> off;
  final int total;
  final int selected;

  const _Donut({
    required this.slices,
    required this.off,
    required this.total,
    required this.selected,
  });

  @override
  Widget build(BuildContext context) {
    final text = formatBytes(selected);
    final parts = text.split(' ');
    return Stack(
      alignment: Alignment.center,
      children: [
        Positioned.fill(
          child: CustomPaint(
            painter: _DonutPainter(slices, off, total),
          ),
        ),
        Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(parts.first,
                style: const TextStyle(
                    fontSize: 40, fontWeight: FontWeight.w700, height: 1)),
            if (parts.length > 1)
              Text(parts.sublist(1).join(' '),
                  style: const TextStyle(color: Colors.white54, fontSize: 15)),
          ],
        ),
        // Foizlar — bo'laklar ustida.
        ..._labels(),
      ],
    );
  }

  List<Widget> _labels() {
    if (total <= 0) return const [];
    final out = <Widget>[];
    var start = -math.pi / 2;
    for (final s in slices) {
      final sweep = 2 * math.pi * s.bytes / total;
      final share = s.bytes / total;
      if (share >= 0.06) {
        final mid = start + sweep / 2;
        // Halqa markaziy chizig'i: radius 120 - 22.
        const r = 98.0;
        out.add(Positioned(
          left: 120 + r * math.cos(mid) - 24,
          top: 120 + r * math.sin(mid) - 12,
          width: 48,
          height: 24,
          child: Center(
            child: Text(_pct(share),
                style: TextStyle(
                    fontSize: 15,
                    fontWeight: FontWeight.w700,
                    color: off.contains(s.label)
                        ? Colors.white38
                        : Colors.white)),
          ),
        ));
      }
      start += sweep;
    }
    return out;
  }
}

class _DonutPainter extends CustomPainter {
  final List<StorageSlice> slices;
  final Set<String> off;
  final int total;

  _DonutPainter(this.slices, Set<String> off, this.total)
      : off = Set.of(off);

  @override
  void paint(Canvas canvas, Size size) {
    const stroke = 44.0;
    final rect = Rect.fromCircle(
        center: size.center(Offset.zero),
        radius: size.shortestSide / 2 - stroke / 2);
    final p = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = stroke;
    if (total <= 0) {
      canvas.drawArc(rect, 0, 2 * math.pi, false,
          p..color = Colors.white10);
      return;
    }
    // Bo'laklar orasida kichik tirqish (Telegram'dagidek).
    final gap = slices.length > 1 ? 0.035 : 0.0;
    var start = -math.pi / 2;
    for (final s in slices) {
      final sweep = 2 * math.pi * s.bytes / total;
      final c = _colorOf(s.label);
      if (sweep > gap) {
        canvas.drawArc(rect, start + gap / 2, sweep - gap, false,
            p..color = off.contains(s.label) ? c.withValues(alpha: 0.25) : c);
      }
      start += sweep;
    }
  }

  @override
  bool shouldRepaint(_DonutPainter old) =>
      old.total != total ||
      old.slices != slices ||
      old.off.length != off.length ||
      !old.off.containsAll(off);
}
