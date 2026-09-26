// lib/screens/storage_screen.dart — "XOTIRADAN FOYDALANISH" OYNASI.
//
// TALAB (foydalanuvchi): "Tozalash oynasini GitHub'dagi Cherrygram
// reposida qanday tuzilgan bo'lsa xuddi shunaqa qilib ber".
//
// Manba: `arsLan4k1390/Cherrygram` —
// `TMessagesProj/src/main/java/org/telegram/ui/CacheControlActivity.java`
// va `Components/CacheChart.java`. Tuzilishi va o'lchamlari aynan
// o'shandan olingan:
//
//   * oq (bu yerda — karta rangi) blok: yuqori panel, halqa diagramma
//     (balandligi 200, diametri 172, qalinligi 38, bo'laklar orasi 2°,
//     bo'lak ichida foiz — 5% dan katta bo'lsa), o'rtada TANLANGAN
//     hajm (raqam 32, birligi 12);
//   * sarlavha "Xotiradan foydalanish", izoh "ARUGRAM qurilma
//     xotirasining N% qismini egallagan" va uning ostidagi chiziq
//     (ilova / boshqa ilovalar / bo'sh joy);
//   * toifalar: dumaloq rangli belgilash, nomi + qalin foiz, o'ngda
//     hajm (qator 50, ajratuvchi chiziq 60 dan boshlanadi); belgisi
//     olingan toifa halqadan butunlay chiqadi;
//   * "Keshni tozalash <hajm>" tugmasi (48, chetlardan 16) —
//     hammasi tanlanmagan bo'lsa "Tanlanganini tozalash";
//   * tasdiqlash oynasi, so'ng yopib bo'lmaydigan "Kesh tozalanmoqda"
//     pardasi (foiz, chiziq);
//   * kesh bo'sh bo'lsa — yashil halqa va belgi, "Xotira tozalandi";
//   * kulrang fonda izoh: fayllar serverda qoladi.
//
// Telegram'dagi "Keshni avtomatik o'chirish" va "Keshning eng katta
// hajmi" bo'limlari bu yerda YO'Q: ilovada ularga mos ish (fon
// tozalovchisi) hali yo'q, ishlamaydigan sozlama esa ko'rsatilmaydi.

import 'dart:async';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:lottie/lottie.dart';

import '../services/format.dart';
import '../services/storage_usage.dart';
import '../widgets/glass.dart';

/// Toifa ranglari (Telegram'dagi `statisticChartLine_*`).
const Map<String, Color> _colors = {
  'Videolar': Color(0xFF3E8CF0),
  'Posterlar': Color(0xFF56B6F5),
  'Stikerlar va emojilar': Color(0xFFF09B3A),
  'GIF va chat fayllari': Color(0xFF4FC76A),
  'Vaqtinchalik fayllar': Color(0xFF9A6FE2),
};

Color _colorOf(String label) => _colors[label] ?? const Color(0xFFE8B730);

/// Telegram'dagi `windowBackgroundWhiteGrayText4`.
const Color _grayText = Color(0xFF8A8A8E);

/// Telegram'dagi `listSelector` — bosilgan qator, yuklanish halqasi.
const Color _selector = Color(0x14FFFFFF);

/// Yashil "tozalandi" gradienti (`CacheChart.completeGradient`).
const Color _green1 = Color(0xFF6ED556);
const Color _green2 = Color(0xFF41BA71);

class StorageScreen extends StatefulWidget {
  const StorageScreen({super.key});

  @override
  State<StorageScreen> createState() => _StorageScreenState();
}

class _StorageScreenState extends State<StorageScreen> {
  final StorageUsageService _svc = StorageUsageService.instance;
  final ScrollController _scroll = ScrollController();

  /// Belgisi OLINGAN toifalar (qolganlari tanlangan).
  final Set<String> _off = {};

  /// Halqada barmoq ostidagi toifa — ro'yxatda qatori yoritiladi.
  String? _highlight;

  /// Sarlavha yuqori panelda (halqa va sarlavha ekrandan chiqqach).
  bool _pinned = false;

  @override
  void initState() {
    super.initState();
    _svc.refresh();
    _scroll.addListener(() {
      final pinned = _scroll.hasClients && _scroll.offset > 230;
      if (pinned != _pinned) setState(() => _pinned = pinned);
    });
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  List<StorageSlice> get _cache => _svc.usage.slices
      .where((s) => StorageUsageService.clearable.contains(s.label))
      .toList();

  int _selectedOf(List<StorageSlice> cache) => cache
      .where((s) => !_off.contains(s.label))
      .fold(0, (a, s) => a + s.bytes);

  bool _allSelected(List<StorageSlice> cache) =>
      cache.every((s) => !_off.contains(s.label));

  String _buttonText(List<StorageSlice> cache) => _allSelected(cache)
      ? 'Keshni tozalash'
      : 'Tanlanganini tozalash';

  // ── TOZALASH (`ClearCacheButtonInternal`) ─────────────────────
  Future<void> _clear() async {
    final cache = _cache;
    final labels = cache
        .where((s) => !_off.contains(s.label))
        .map((s) => s.label)
        .toSet();
    final size = _selectedOf(cache);
    if (labels.isEmpty || size <= 0) return;
    final action = _buttonText(cache);
    final ok = await showDialog<bool>(
      context: context,
      builder: (c) => AlertDialog(
        backgroundColor: AppColors.card,
        title: Text('Keshni tozalash (${formatBytes(size)})',
            style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700)),
        content: const Text(_infoText,
            style: TextStyle(color: Colors.white70, fontSize: 16)),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(c, false),
              child: const Text('Bekor qilish')),
          TextButton(
              style: TextButton.styleFrom(
                  foregroundColor: const Color(0xFFFF5A5A)),
              onPressed: () => Navigator.pop(c, true),
              child: Text(action)),
        ],
      ),
    );
    if (ok != true || !mounted) return;

    // TALAB (foydalanuvchi): "Tozalash tugmasini bosganda Telegram'dagidek
    // jo'ja supurgida supurayotgan animatsiyasi chiqsin". Parda DARHOL
    // chiqadi va tozalash tez tugasa ham animatsiya ko'rinib ulgurishi
    // uchun kamida 1.6 soniya turadi.
    final progress = ValueNotifier<double>(0);
    var done = false;
    var shownAt = -1;
    final sheetClosed = Completer<void>();
    Timer(Duration.zero, () {
      if (done || !mounted) {
        if (!sheetClosed.isCompleted) sheetClosed.complete();
        return;
      }
      shownAt = DateTime.now().millisecondsSinceEpoch;
      showModalBottomSheet<void>(
        context: context,
        isDismissible: false,
        enableDrag: false,
        backgroundColor: AppColors.card,
        shape: const RoundedRectangleBorder(
            borderRadius: BorderRadius.vertical(top: Radius.circular(12))),
        builder: (_) => PopScope(
          canPop: false,
          child: _ClearingView(progress: progress),
        ),
      ).whenComplete(() {
        if (!sheetClosed.isCompleted) sheetClosed.complete();
      });
    });

    await _svc.clear(labels, onProgress: (v) => progress.value = v);
    done = true;
    progress.value = 1;
    if (shownAt > 0 && mounted) {
      final left = 1600 - (DateTime.now().millisecondsSinceEpoch - shownAt);
      if (left > 0) await Future<void>.delayed(Duration(milliseconds: left));
      if (mounted) Navigator.of(context).pop();
    }
    await sheetClosed.future.timeout(const Duration(seconds: 2),
        onTimeout: () {});
    progress.dispose();
    if (!mounted) return;
    setState(_off.clear);
  }

  @override
  Widget build(BuildContext context) {
    final bottom = MediaQuery.of(context).padding.bottom;
    return Scaffold(
      backgroundColor: AppColors.bg,
      appBar: AppBar(
        backgroundColor: AppColors.card,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
        title: AnimatedOpacity(
          opacity: _pinned ? 1 : 0,
          duration: const Duration(milliseconds: 200),
          child: const Text('Xotiradan foydalanish',
              style: TextStyle(fontSize: 20, fontWeight: FontWeight.w600)),
        ),
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(1),
          child: AnimatedOpacity(
            opacity: _pinned ? 1 : 0,
            duration: const Duration(milliseconds: 200),
            child: Container(height: 1, color: Colors.black),
          ),
        ),
      ),
      body: AnimatedBuilder(
        animation: _svc,
        builder: (context, _) {
          final calculating = !_svc.measured;
          final cache = _cache;
          final total = cache.fold<int>(0, (a, s) => a + s.bytes);
          final selected = _selectedOf(cache);
          final hasCache = calculating || total > 0;
          final percents = _roundPercents(
              [for (final s in cache) total > 0 ? s.bytes / total : 0.0]);

          return ListView(
            controller: _scroll,
            padding: EdgeInsets.only(bottom: bottom + 16),
            children: [
              ColoredBox(
                color: AppColors.card,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _CacheChart(
                      loading: calculating,
                      complete: !calculating && total <= 0,
                      slices: [
                        for (final s in cache)
                          if (!_off.contains(s.label)) s,
                      ],
                      selectedBytes: selected,
                      onPress: (l) => setState(() => _highlight = l),
                    ),
                    _ChartHeader(
                      calculating: calculating,
                      hasCache: hasCache,
                      cacheBytes: total,
                      deviceTotal: _svc.deviceTotal,
                      deviceFree: _svc.deviceFree,
                    ),
                    if (calculating)
                      for (var i = 0; i < 5; i++) const _LoadingRow()
                    else
                      for (var i = 0; i < cache.length; i++)
                        _SectionRow(
                          slice: cache[i],
                          percent: percents[i],
                          checked: !_off.contains(cache[i].label),
                          divider: i < cache.length - 1,
                          highlighted: _highlight == cache[i].label,
                          onTap: () => setState(() {
                            final l = cache[i].label;
                            if (!_off.remove(l)) _off.add(l);
                          }),
                        ),
                    if (hasCache)
                      _ClearButton(
                        text: _buttonText(cache),
                        value: selected > 0 ? formatBytes(selected) : '',
                        enabled: !calculating && selected > 0,
                        onTap: _clear,
                      ),
                  ],
                ),
              ),
              if (hasCache) const _InfoCell(_infoText),
            ],
          );
        },
      ),
    );
  }
}

const String _infoText =
    'Barcha videolar, rasmlar, stikerlar va emojilar serverda qoladi — '
    'kerak bo\'lsa ularni qayta yuklab olishingiz mumkin.';

/// Foizlarni jami 100 bo'ladigan qilib yaxlitlaydi
/// (`AndroidUtilities.roundPercents` — eng katta qoldiq usuli).
List<int> _roundPercents(List<double> shares) {
  final raw = [for (final s in shares) s * 100];
  final out = [for (final r in raw) r.floor()];
  final sum = raw.fold<double>(0, (a, b) => a + b);
  var left = sum.round() - out.fold<int>(0, (a, b) => a + b);
  final order = List<int>.generate(raw.length, (i) => i)
    ..sort((a, b) => (raw[b] - out[b]).compareTo(raw[a] - out[a]));
  for (final i in order) {
    if (left <= 0) break;
    out[i]++;
    left--;
  }
  return out;
}

// ══════════════════════════════════════════════════════════════
//  HALQA DIAGRAMMA (`CacheChart`)
// ══════════════════════════════════════════════════════════════

class _CacheChart extends StatefulWidget {
  final bool loading;
  final bool complete;

  /// Faqat TANLANGAN toifalar (belgisi olingani halqada yo'q).
  final List<StorageSlice> slices;
  final int selectedBytes;
  final ValueChanged<String?> onPress;

  const _CacheChart({
    required this.loading,
    required this.complete,
    required this.slices,
    required this.selectedBytes,
    required this.onPress,
  });

  @override
  State<_CacheChart> createState() => _CacheChartState();
}

/// Bitta bo'lakning holati: markaz burchagi va yarim kengligi
/// (gradusda, Telegram'dagidek), foiz matni.
class _Sector {
  final double center;
  final double half;
  final String text;
  final double textAlpha;
  final double textScale;

  const _Sector(this.center, this.half,
      [this.text = '', this.textAlpha = 0, this.textScale = 1]);
}

class _CacheChartState extends State<_CacheChart>
    with TickerProviderStateMixin {
  static const double _separator = 2;

  /// Bo'laklar siljishi (650 ms, `EASE_OUT_QUINT`).
  late final AnimationController _move = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 650))
    ..value = 1;

  /// Bosilgan bo'lak 9 px ga kattalashadi (200 ms).
  late final AnimationController _press = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 200));

  /// Zarrachalar va yuklanish aylanishi uchun vaqt.
  late final AnimationController _clock =
      AnimationController(vsync: this, duration: const Duration(seconds: 10))
        ..repeat();

  Map<String, _Sector> _from = {};
  Map<String, _Sector> _to = {};
  String? _pressed;

  @override
  void initState() {
    super.initState();
    _to = _layout(widget.slices);
    _from = _to;
  }

  @override
  void didUpdateWidget(_CacheChart old) {
    super.didUpdateWidget(old);
    final next = _layout(widget.slices);
    if (!_sameLayout(next, _to)) {
      _from = _current();
      _to = next;
      _move.forward(from: 0);
    }
  }

  @override
  void dispose() {
    _move.dispose();
    _press.dispose();
    _clock.dispose();
    super.dispose();
  }

  bool _sameLayout(Map<String, _Sector> a, Map<String, _Sector> b) {
    if (a.length != b.length) return false;
    for (final e in a.entries) {
      final o = b[e.key];
      if (o == null ||
          (o.center - e.value.center).abs() > 0.01 ||
          (o.half - e.value.half).abs() > 0.01 ||
          o.text != e.value.text) {
        return false;
      }
    }
    return true;
  }

  /// Hozirgi (oraliq) holat — yangi siljish shu joydan boshlanadi.
  Map<String, _Sector> _current() {
    final t = Curves.easeOutQuint.transform(_move.value);
    final out = <String, _Sector>{};
    for (final k in {..._from.keys, ..._to.keys}) {
      final a = _from[k];
      final b = _to[k];
      final s = _lerp(a, b, t);
      if (s.half > 0.001) out[k] = s;
    }
    return out;
  }

  static _Sector _lerp(_Sector? a, _Sector? b, double t) {
    // Yangi bo'lak o'z joyida noldan o'sadi, ketayotgani o'z joyida
    // yo'qoladi.
    a ??= _Sector(b!.center, 0, b.text, 0, b.textScale);
    b ??= _Sector(a.center, 0, a.text, 0, a.textScale);
    return _Sector(
      _lerpAngle(a.center, b.center, t),
      ui.lerpDouble(a.half, b.half, t)!,
      t < 0.5 ? a.text : b.text,
      ui.lerpDouble(a.textAlpha, b.textAlpha, t)!,
      ui.lerpDouble(a.textScale, b.textScale, t)!,
    );
  }

  static double _lerpAngle(double a, double b, double f) =>
      (a + (((b - a + 360 + 180) % 360) - 180) * f + 360) % 360;

  /// `CacheChart.setSegments` — burchaklar hisobi.
  static Map<String, _Sector> _layout(List<StorageSlice> slices) {
    final list = slices.where((s) => s.bytes > 0).toList();
    final sum = list.fold<int>(0, (a, s) => a + s.bytes);
    if (sum <= 0) return {};
    var under = 0;
    var minus = 0.0;
    for (final s in list) {
      final p = s.bytes / sum;
      if (p > 0 && p < .02) {
        under++;
        minus += p;
      }
    }
    final percents = _roundPercents([for (final s in list) s.bytes / sum]);
    final pct = {
      for (var i = 0; i < list.length; i++) list[i].label: percents[i]
    };
    // Kichigidan kattasiga (Telegram'dagidek).
    list.sort((a, b) => a.bytes.compareTo(b.bytes));
    final count = list.length;
    final total = 360 - _separator * (count < 2 ? 0 : count);
    final out = <String, _Sector>{};
    var prev = 0.0;
    var k = 0;
    for (final s in list) {
      var p = s.bytes / sum;
      final textAlpha = p > .05 && p < 1 ? 1.0 : 0.0;
      final textScale = p < .08 || pct[s.label]! >= 100 ? .85 : 1.0;
      if (p < .02) {
        p = .02;
      } else {
        p *= 1 - (.02 * under - minus);
      }
      final from = prev * total + k * _separator;
      final to = from + p * total;
      out[s.label] = _Sector(
          (from + to) / 2, (to - from).abs() / 2, '${pct[s.label]}%',
          textAlpha, textScale);
      prev += p;
      k++;
    }
    return out;
  }

  // ── BARMOQ (`dispatchTouchEvent`) ─────────────────────────────
  String? _hit(Offset pos, Size size) {
    final c = Offset(size.width / 2, size.height / 2);
    final d = pos - c;
    final r = d.distance;
    if (r <= _ChartPainter.radius - _ChartPainter.thickness ||
        r >= _ChartPainter.radius + 14) {
      return null;
    }
    var a = math.atan2(d.dy, d.dx) * 180 / math.pi;
    if (a < 0) a += 360;
    for (final e in _to.entries) {
      if (a >= e.value.center - e.value.half &&
          a <= e.value.center + e.value.half) {
        return e.key;
      }
    }
    return null;
  }

  /// Barmoq hozir qaysi bo'lak ustida (`_pressed` esa kichrayish
  /// tugaguncha oxirgi bosilganini eslab turadi).
  String? _target;

  void _setPressed(String? label) {
    if (label == _target) return;
    _target = label;
    widget.onPress(label);
    if (label == null) {
      _press.reverse();
      return;
    }
    setState(() => _pressed = label);
    _press.forward(from: 0);
  }

  @override
  Widget build(BuildContext context) {
    final text = formatBytes(widget.selectedBytes).split(' ');
    return SizedBox(
      height: 200,
      child: LayoutBuilder(builder: (context, box) {
        final size = Size(box.maxWidth, 200);
        final interactive = !widget.loading && !widget.complete;
        return GestureDetector(
          behavior: HitTestBehavior.opaque,
          onPanDown: interactive
              ? (d) => _setPressed(_hit(d.localPosition, size))
              : null,
          onPanUpdate: interactive
              ? (d) => _setPressed(_hit(d.localPosition, size))
              : null,
          onPanEnd: interactive ? (_) => _setPressed(null) : null,
          onPanCancel: interactive ? () => _setPressed(null) : null,
          child: RepaintBoundary(
            child: CustomPaint(
              size: size,
              painter: _ChartPainter(
                repaint: Listenable.merge([_move, _press, _clock]),
                sectors: () => _current(),
                pressed: _pressed,
                pressT: () => _press.value,
                clock: () =>
                    (_clock.lastElapsedDuration?.inMicroseconds ?? 0) / 1e7,
                loading: widget.loading,
                complete: widget.complete,
                top: widget.loading ? '' : text.first,
                bottom: widget.loading || text.length < 2
                    ? ''
                    : text.sublist(1).join(' '),
              ),
            ),
          ),
        );
      }),
    );
  }
}

class _ChartPainter extends CustomPainter {
  /// Diametri 172 -> radiusi 86; qalinligi 38.
  static const double radius = 86;
  static const double thickness = 38;

  final Map<String, _Sector> Function() sectors;
  final String? pressed;
  final double Function() pressT;
  final double Function() clock;
  final bool loading;
  final bool complete;
  final String top;
  final String bottom;

  _ChartPainter({
    required Listenable repaint,
    required this.sectors,
    required this.pressed,
    required this.pressT,
    required this.clock,
    required this.loading,
    required this.complete,
    required this.top,
    required this.bottom,
  }) : super(repaint: repaint);

  @override
  void paint(Canvas canvas, Size size) {
    final c = Offset(size.width / 2, size.height / 2);
    if (loading) {
      _paintLoading(canvas, c);
      return;
    }
    if (complete) {
      _paintComplete(canvas, c);
      return;
    }
    final time = clock();
    final all = sectors();
    // Kichikdan kattaga, bosilgani eng ustida.
    final keys = all.keys.toList()
      ..sort((a, b) => (a == pressed ? 1 : 0) - (b == pressed ? 1 : 0));
    for (final label in keys) {
      final s = all[label]!;
      if (s.half <= 0) continue;
      final grow = label == pressed ? 9 * pressT() : 0.0;
      _paintSector(canvas, c, s, _colorOf(label), radius + grow, time);
    }
    _paintCenter(canvas, c);
  }

  void _paintSector(Canvas canvas, Offset c, _Sector s, Color color,
      double outer, double time) {
    const inner = radius - thickness;
    final full = s.half * 2 >= 359;
    final path = Path();
    if (full) {
      path
        ..fillType = PathFillType.evenOdd
        ..addOval(Rect.fromCircle(center: c, radius: outer))
        ..addOval(Rect.fromCircle(center: c, radius: inner));
    } else {
      final from = (s.center - s.half) * math.pi / 180;
      final sweep = s.half * 2 * math.pi / 180;
      path
        ..arcTo(Rect.fromCircle(center: c, radius: outer), from, sweep, true)
        ..arcTo(Rect.fromCircle(center: c, radius: inner), from + sweep,
            -sweep, false)
        ..close();
    }
    // Rang markazdan chetga to'yinadi (`RadialGradient`, 0.3..1).
    final light = Color.alphaBlend(const Color(0x30FFFFFF), color);
    final dark = Color.alphaBlend(const Color(0x03000000), color);
    canvas.drawPath(
        path,
        Paint()
          ..shader = ui.Gradient.radial(
              c, radius, [light, dark], const [.3, 1]));

    // Zarrachalar — bo'lak ichida ichkaridan tashqariga oqadi.
    canvas.save();
    canvas.clipPath(path);
    final textPos = c +
        Offset(math.cos(s.center * math.pi / 180),
                math.sin(s.center * math.pi / 180)) *
            ((outer + inner) / 2);
    _paintParticles(canvas, c, s, inner, outer, time, textPos);
    canvas.restore();

    if (s.textAlpha > 0.01 && s.text.isNotEmpty) {
      final tp = TextPainter(
        text: TextSpan(
          text: s.text,
          style: TextStyle(
            color: Colors.white.withValues(alpha: s.textAlpha),
            fontSize: 15 * s.textScale,
            fontWeight: FontWeight.w700,
          ),
        ),
        textDirection: TextDirection.ltr,
      )..layout();
      tp.paint(canvas, textPos - Offset(tp.width / 2, tp.height / 2));
    }
  }

  void _paintParticles(Canvas canvas, Offset c, _Sector s, double inner,
      double outer, double time, Offset textPos) {
    const step = 7.0;
    const sz = 5.0;
    final sqrt2 = math.sqrt2;
    final start = (s.center - s.half) % 360;
    final end = start + s.half * 2;
    final paint = Paint();
    for (var i = (start / step).floor(); i <= (end / step).ceil(); i++) {
      final angle = i * step;
      final t = ((time + 100) * (1 + (math.sin(angle * 2000) + 1) * .25)) % 1;
      final r = ui.lerpDouble(inner - sz * sqrt2, outer + sz * sqrt2, t)!;
      final rad = angle * math.pi / 180;
      final p = c + Offset(math.cos(rad), math.sin(rad)) * r;
      final wave = .25 * (math.sin(t * math.pi) - 1) + 1;
      final nearText =
          s.textAlpha > 0 ? math.min((p - textPos).distance / 64, 1.0) : 1.0;
      final alpha =
          (.65 * (-1.75 * (t - .5).abs() + 1) * wave * nearText).clamp(0.0, 1.0);
      if (alpha <= 0) continue;
      final scale = .75 * wave * (.8 + (math.sin(angle) + 1) * .25);
      paint.color = Colors.white.withValues(alpha: alpha);
      canvas.drawCircle(p, 2.4 * scale, paint);
    }
  }

  void _paintCenter(Canvas canvas, Offset c) {
    final tp = TextPainter(
      text: TextSpan(
          text: top,
          style: const TextStyle(
              color: Colors.white,
              fontSize: 32,
              fontWeight: FontWeight.w700,
              height: 1)),
      textDirection: TextDirection.ltr,
    )..layout();
    tp.paint(canvas, c - Offset(tp.width / 2, tp.height / 2 + 5));
    final bp = TextPainter(
      text: TextSpan(
          text: bottom,
          style: const TextStyle(color: _grayText, fontSize: 12, height: 1)),
      textDirection: TextDirection.ltr,
    )..layout();
    bp.paint(canvas, c + Offset(-bp.width / 2, 22 - bp.height / 2));
  }

  void _paintLoading(Canvas canvas, Offset c) {
    final rect = Rect.fromCircle(center: c, radius: radius - thickness / 2);
    final stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = thickness;
    canvas.drawCircle(c, radius - thickness / 2, stroke..color = _selector);
    // Aylanib yuruvchi yoy (`CircularProgressDrawable`).
    final t = clock() * 10;
    final start = (t * 2 * math.pi) % (2 * math.pi);
    final sweep = math.pi * (.35 + .25 * math.sin(t * math.pi));
    canvas.drawArc(
        rect,
        start,
        sweep,
        false,
        stroke
          ..color = const Color(0x1FFFFFFF)
          ..strokeCap = StrokeCap.round);
  }

  void _paintComplete(Canvas canvas, Offset c) {
    const w = 10.0;
    const d = radius * 2;
    final bounds = Rect.fromCircle(center: c, radius: radius);
    final shader = ui.Gradient.linear(
      Offset(0, c.dy - 100),
      Offset(0, c.dy + 100),
      const [Color(0x006ED556), _green1, _green2, Color(0x0041BA71)],
      const [0, .07, .93, 1],
    );
    final paint = Paint()
      ..shader = shader
      ..style = PaintingStyle.stroke
      ..strokeWidth = w
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    canvas.drawCircle(c, radius - w / 2, paint);
    final check = Path()
      ..moveTo(bounds.left + d * .348, bounds.top + d * .538)
      ..lineTo(bounds.left + d * .447, bounds.top + d * .636)
      ..lineTo(bounds.left + d * .678, bounds.top + d * .402);
    canvas.drawPath(check, paint);
  }

  @override
  bool shouldRepaint(_ChartPainter old) =>
      old.pressed != pressed ||
      old.loading != loading ||
      old.complete != complete ||
      old.top != top ||
      old.bottom != bottom ||
      old.sectors != sectors;
}

// ══════════════════════════════════════════════════════════════
//  SARLAVHA (`CacheChartHeader`)
// ══════════════════════════════════════════════════════════════

class _ChartHeader extends StatelessWidget {
  final bool calculating;
  final bool hasCache;
  final int cacheBytes;
  final int deviceTotal;
  final int deviceFree;

  const _ChartHeader({
    required this.calculating,
    required this.hasCache,
    required this.cacheBytes,
    required this.deviceTotal,
    required this.deviceFree,
  });

  String _subtitle() {
    if (calculating) return 'Hisoblanmoqda...';
    if (!hasCache) {
      return 'Keshdagi barcha fayllar o\'chirildi. Ular kerak bo\'lganda '
          'serverdan qayta yuklab olinadi.';
    }
    if (deviceTotal <= 0) {
      return 'ARUGRAM keshi qurilmada ${formatBytes(cacheBytes)} joy '
          'egallagan.';
    }
    final pct = cacheBytes / deviceTotal * 100;
    if (pct < 1) {
      return 'ARUGRAM qurilma xotirasining 1% dan kamroq qismini '
          'egallagan.';
    }
    final s = pct < 10
        ? pct.toStringAsFixed(1).replaceAll('.', ',').replaceAll(',0', '')
        : pct.round().toString();
    return 'ARUGRAM qurilma xotirasining $s% qismini egallagan.';
  }

  @override
  Widget build(BuildContext context) {
    final showBar = hasCache && (calculating || deviceTotal > 0);
    final percent = deviceTotal > 0 ? cacheBytes / deviceTotal : 0.0;
    final used = deviceTotal > 0 && deviceFree > 0
        ? (deviceTotal - deviceFree) / deviceTotal
        : 0.0;
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        children: [
          AnimatedSwitcher(
            duration: const Duration(milliseconds: 340),
            child: Text(
              hasCache ? 'Xotiradan foydalanish' : 'Xotira tozalandi',
              key: ValueKey(hasCache),
              textAlign: TextAlign.center,
              style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700),
            ),
          ),
          const SizedBox(height: 6),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24),
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 340),
              child: Text(
                _subtitle(),
                key: ValueKey(_subtitle()),
                textAlign: TextAlign.center,
                style: const TextStyle(color: _grayText, fontSize: 13),
              ),
            ),
          ),
          if (showBar) ...[
            const SizedBox(height: 12),
            LayoutBuilder(builder: (context, box) {
              final w = math.min(174.0, box.maxWidth * .8);
              return SizedBox(
                width: w,
                height: 4,
                child: CustomPaint(
                  painter: _UsageBarPainter(
                    loading: calculating,
                    percent: percent,
                    used: used,
                  ),
                ),
              );
            }),
            const SizedBox(height: 18),
          ] else
            const SizedBox(height: 14),
        ],
      ),
    );
  }
}

/// Ilova ulushi (urg'u rangi), boshqa ilovalar (xira urg'u) va bo'sh
/// joy (kulrang) — `CacheChartHeader.dispatchDraw`.
class _UsageBarPainter extends CustomPainter {
  final bool loading;
  final double percent;
  final double used;

  _UsageBarPainter({
    required this.loading,
    required this.percent,
    required this.used,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final w = size.width;
    final h = size.height;
    void bar(double from, double to, Color color, double l, double r) {
      if (to - from <= 0) return;
      canvas.drawRRect(
        RRect.fromLTRBAndCorners(from, 0, to, h,
            topLeft: Radius.circular(l),
            bottomLeft: Radius.circular(l),
            topRight: Radius.circular(r),
            bottomRight: Radius.circular(r)),
        Paint()..color = color,
      );
    }

    if (loading) {
      bar(0, w, _selector, 2, 2);
      return;
    }
    final a = math.max(4.0, percent * w);
    final b = math.max(4.0, used * w);
    bar(math.max(a, b) + 1, w, _selector, 1, 2);
    bar(a + 1, b, Color.lerp(AppColors.accent2, AppColors.card, .75)!, 1,
        used > .97 ? 2 : 1);
    bar(0, a, AppColors.accent2, 2, percent > .97 ? 2 : 1);
  }

  @override
  bool shouldRepaint(_UsageBarPainter old) =>
      old.loading != loading || old.percent != percent || old.used != used;
}

// ══════════════════════════════════════════════════════════════
//  TOIFA QATORI (`CheckBoxCell`, dumaloq belgilash)
// ══════════════════════════════════════════════════════════════

class _SectionRow extends StatelessWidget {
  final StorageSlice slice;
  final int percent;
  final bool checked;
  final bool divider;
  final bool highlighted;
  final VoidCallback onTap;

  const _SectionRow({
    required this.slice,
    required this.percent,
    required this.checked,
    required this.divider,
    required this.highlighted,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final c = _colorOf(slice.label);
    return Material(
      color: highlighted ? _selector : Colors.transparent,
      child: InkWell(
        onTap: onTap,
        child: SizedBox(
          height: 50,
          child: Stack(
            children: [
              Positioned.fill(
                child: Row(
                  children: [
                    const SizedBox(width: 21),
                    _RoundCheck(checked: checked, color: c),
                    const SizedBox(width: 18),
                    Expanded(
                      child: Text.rich(
                        TextSpan(children: [
                          TextSpan(text: slice.label),
                          TextSpan(
                            text: '  ${percent <= 0 ? '<1' : percent}%',
                            style: const TextStyle(
                                fontSize: 13.3, fontWeight: FontWeight.w700),
                          ),
                        ]),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(fontSize: 16),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Text(formatBytes(slice.bytes),
                        style: const TextStyle(
                            color: AppColors.accent2, fontSize: 16)),
                    const SizedBox(width: 21),
                  ],
                ),
              ),
              if (divider)
                const Positioned(
                  left: 60,
                  right: 0,
                  bottom: 0,
                  child: SizedBox(
                      height: 1, child: ColoredBox(color: Colors.black)),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

/// `CheckBox2` — dumaloq, belgilanganda toifa rangi bilan to'ladi.
class _RoundCheck extends StatelessWidget {
  final bool checked;
  final Color color;

  const _RoundCheck({required this.checked, required this.color});

  @override
  Widget build(BuildContext context) {
    return AnimatedContainer(
      duration: const Duration(milliseconds: 200),
      width: 21,
      height: 21,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: checked ? color : Colors.transparent,
        border: Border.all(color: checked ? color : _grayText, width: 1.5),
      ),
      child: AnimatedScale(
        duration: const Duration(milliseconds: 200),
        scale: checked ? 1 : 0,
        child: const Icon(Icons.check_rounded, size: 15, color: Colors.white),
      ),
    );
  }
}

/// Hisoblanayotganda — qator o'rnida xira shakl (`FlickerLoadingView`).
class _LoadingRow extends StatelessWidget {
  const _LoadingRow();

  @override
  Widget build(BuildContext context) {
    Widget block(double w, double h) => Container(
          width: w,
          height: h,
          decoration: BoxDecoration(
            color: _selector,
            borderRadius: BorderRadius.circular(h / 2),
          ),
        );
    return SizedBox(
      height: 50,
      child: Row(
        children: [
          const SizedBox(width: 21),
          block(21, 21),
          const SizedBox(width: 18),
          block(140, 10),
          const Spacer(),
          block(48, 10),
          const SizedBox(width: 21),
        ],
      ),
    );
  }
}

// ══════════════════════════════════════════════════════════════
//  TOZALASH TUGMASI (`ClearCacheButton`)
// ══════════════════════════════════════════════════════════════

class _ClearButton extends StatelessWidget {
  final String text;
  final String value;
  final bool enabled;
  final VoidCallback onTap;

  const _ClearButton({
    required this.text,
    required this.value,
    required this.enabled,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 21, 16, 16),
      child: AnimatedOpacity(
        duration: const Duration(milliseconds: 200),
        opacity: enabled ? 1 : .5,
        child: Material(
          color: AppColors.accent2,
          borderRadius: BorderRadius.circular(24),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: enabled ? onTap : null,
            child: SizedBox(
              height: 48,
              child: Center(
                child: Text.rich(
                  TextSpan(children: [
                    TextSpan(text: text),
                    if (value.isNotEmpty)
                      TextSpan(
                        text: '  $value',
                        style: TextStyle(
                            color: Color.alphaBlend(
                                Colors.white.withValues(alpha: .7),
                                AppColors.accent2)),
                      ),
                  ]),
                  style: const TextStyle(
                      color: Colors.white,
                      fontSize: 14,
                      fontWeight: FontWeight.w700),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Kulrang fondagi izoh (`TextInfoPrivacyCell`).
class _InfoCell extends StatelessWidget {
  final String text;

  const _InfoCell(this.text);

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(21, 10, 21, 17),
      child: Text(text,
          style: const TextStyle(color: _grayText, fontSize: 14, height: 1.3)),
    );
  }
}

// ══════════════════════════════════════════════════════════════
//  "KESH TOZALANMOQDA" PARDASI (`ClearingCacheView`)
// ══════════════════════════════════════════════════════════════

class _ClearingView extends StatelessWidget {
  final ValueNotifier<double> progress;

  const _ClearingView({required this.progress});

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      top: false,
      child: Padding(
        padding: const EdgeInsets.only(bottom: 28),
        child: ValueListenableBuilder<double>(
          valueListenable: progress,
          builder: (context, v, _) {
            final p = v.clamp(0.0, 1.0);
            return Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const SizedBox(height: 16),
                // Telegram `utyan_cache`: supurgi bilan supurayotgan
                // jo'ja (takrorlanib o'ynaydi).
                SizedBox(
                  width: 150,
                  height: 150,
                  child: Lottie.asset(
                    'assets/tg_anim/utyan_cache.json',
                    repeat: true,
                    frameRate: FrameRate.max,
                  ),
                ),
                const SizedBox(height: 10),
                Text('${(p * 100).ceil()}%',
                    style: const TextStyle(
                        fontSize: 24, fontWeight: FontWeight.w700)),
                const SizedBox(height: 16),
                SizedBox(
                  width: 240,
                  height: 5,
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(3),
                    child: Stack(
                      children: [
                        Positioned.fill(
                          child: ColoredBox(
                              color: AppColors.accent2.withValues(alpha: .2)),
                        ),
                        TweenAnimationBuilder<double>(
                          tween: Tween(end: p),
                          duration: const Duration(milliseconds: 350),
                          curve: Curves.easeOut,
                          builder: (context, t, _) => FractionallySizedBox(
                            widthFactor: t,
                            heightFactor: 1,
                            alignment: Alignment.centerLeft,
                            child:
                                const ColoredBox(color: AppColors.accent2),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 30),
                const Text('Kesh tozalanmoqda',
                    style:
                        TextStyle(fontSize: 16, fontWeight: FontWeight.w700)),
                const SizedBox(height: 10),
                const SizedBox(
                  width: 240,
                  child: Text(
                    'Kesh tozalanayotganda bu oynani yopmang.',
                    textAlign: TextAlign.center,
                    style: TextStyle(fontSize: 14, color: Colors.white70),
                  ),
                ),
              ],
            );
          },
        ),
      ),
    );
  }
}
