// lib/widgets/tg_composer.dart — TELEGRAM'DAGIDEK YOZISH PANELI.
//
// TALAB (foydalanuvchi): "Telegram'ning pastki yozish va uchchala
// to'plamni ochadigan oynasi qanday ishlasa, xuddi shunday qilib
// yasab ber" (Emoji / GIF / Stikerlar); "premium emoji'ni faqat
// premium'i bor odam yubora olsin".
//
// ── QANDAY ISHLAYDI (Telegram Android bilan bir xil) ────────────
//
//   * Yozish maydonining chap tomonidagi 🙂 tugmasi klaviatura
//     o'rniga PANELNI ochadi (balandligi — klaviaturaniki), tugma
//     ⌨ ga aylanadi; yana bosilsa yoki maydonga bosilsa — klaviatura.
//   * Panelning pastida suzib turgan "Emoji | GIF | Stikerlar"
//     tugmalari, sahifalar yon tomonga suriladi.
//   * Emoji: tepada bo'limlar qatori (yaqinda, kulgichlar, hayvonlar
//     ... bayroqlar, keyin maxsus emoji to'plamlari), bitta uzun
//     ro'yxat; o'ng pastda ⌫ tugmasi. Maxsus emoji to'plamlari
//     Premium'siz odamga 🔒 bilan ko'rinadi, lekin yuborilmaydi.
//   * GIF: qidiruv (`@gif`), saqlanganlar va mashhurlar.
//   * Stikerlar: yaqinda, sevimlilar va o'rnatilgan to'plamlar.
//
// Maxsus emoji yozish maydonining O'ZIDA rasm bo'lib ko'rinadi
// (`TgTextController`); yuborilganda matnga `[ce:<id>:<emoji>]`
// belgisi bo'lib kiradi.

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../services/tg_media.dart';
import 'glass.dart';
import 'tg_emoji_data.dart';
import 'tg_media_view.dart';

// ═══════════════════════════════════════════════════════════════
//  YOZISH MAYDONI BOSHQARUVCHISI
// ═══════════════════════════════════════════════════════════════

/// Maxsus emoji'ni maydonning O'ZIDA rasm qilib ko'rsatadi.
///
/// Har bir maxsus emoji matnda BITTA "shaxsiy" belgi (U+E000..U+F8FF)
/// bo'lib turadi — kursor, o'chirish va belgilash oddiy harfdagidek
/// ishlaydi. Yuborishda [encoded] ularni `[ce:<id>:<emoji>]` ga
/// almashtiradi.
class TgTextController extends TextEditingController {
  TgTextController({super.text});

  final Map<int, TgDoc> _custom = {};
  int _next = 0xE000;

  bool _isCustom(int unit) => _custom.containsKey(unit);

  /// Kursor turgan joyga matn qo'yadi (belgilangan qism almashadi).
  void insertText(String s) {
    final sel = selection;
    final t = text;
    final start = sel.isValid ? sel.start : t.length;
    final end = sel.isValid ? sel.end : t.length;
    value = TextEditingValue(
      text: t.replaceRange(start, end, s),
      selection: TextSelection.collapsed(offset: start + s.length),
    );
  }

  void insertCustom(TgDoc d) {
    if (_next > 0xF8FF) _next = 0xE000;
    final unit = _next++;
    _custom[unit] = d;
    insertText(String.fromCharCode(unit));
  }

  /// ⌫ — kursordan oldingi BITTA ko'rinadigan belgi (emoji o'rtasidan
  /// kesilmaydi).
  void backspace() {
    final sel = selection;
    final t = text;
    var end = sel.isValid ? sel.end : t.length;
    var start = sel.isValid ? sel.start : t.length;
    if (start != end) {
      value = TextEditingValue(
        text: t.replaceRange(start, end, ''),
        selection: TextSelection.collapsed(offset: start),
      );
      return;
    }
    if (end == 0) return;
    final before = t.substring(0, end).characters;
    final last = before.last;
    start = end - last.length;
    value = TextEditingValue(
      text: t.replaceRange(start, end, ''),
      selection: TextSelection.collapsed(offset: start),
    );
  }

  /// Serverga yuboriladigan matn.
  String get encoded {
    final b = StringBuffer();
    for (final unit in text.codeUnits) {
      final d = _custom[unit];
      if (d == null) {
        b.writeCharCode(unit);
      } else {
        final alt = d.emoji.replaceAll(']', '').trim();
        b.write('[ce:${d.id}:${alt.isEmpty ? '⭐' : alt}]');
      }
    }
    return b.toString();
  }

  bool get isBlank => text.trim().isEmpty;

  @override
  void clear() {
    super.clear();
    _custom.clear();
  }

  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    if (!text.codeUnits.any(_isCustom)) {
      return super.buildTextSpan(
          context: context, style: style, withComposing: withComposing);
    }
    final size = (style?.fontSize ?? 14) * 1.3;
    final children = <InlineSpan>[];
    final buf = StringBuffer();
    void flush() {
      if (buf.isEmpty) return;
      children.add(TextSpan(text: buf.toString()));
      buf.clear();
    }

    for (final unit in text.codeUnits) {
      final d = _custom[unit];
      if (d == null) {
        buf.writeCharCode(unit);
      } else {
        flush();
        children.add(WidgetSpan(
          alignment: PlaceholderAlignment.middle,
          child: TgStickerView(doc: d, size: size),
        ));
      }
    }
    flush();
    return TextSpan(style: style, children: children);
  }
}

// ═══════════════════════════════════════════════════════════════
//  KLAVIATURA <-> PANEL
// ═══════════════════════════════════════════════════════════════

/// Yozish qatori + uning ostidagi panel. [row] ga 🙂/⌨ tugmasi
/// beriladi — chaqiruvchi uni o'z qatoriga qo'yadi.
class TgInputArea extends StatefulWidget {
  final TgTextController controller;
  final FocusNode focus;
  final Widget Function(BuildContext context, Widget emojiButton) row;
  final ValueChanged<TgDoc> onSticker;
  final ValueChanged<TgDoc> onGif;

  const TgInputArea({
    super.key,
    required this.controller,
    required this.focus,
    required this.row,
    required this.onSticker,
    required this.onGif,
  });

  @override
  State<TgInputArea> createState() => _TgInputAreaState();
}

class _TgInputAreaState extends State<TgInputArea>
    with WidgetsBindingObserver {
  bool _open = false;

  /// Oxirgi ko'rilgan klaviatura balandligi — panel xuddi shunday.
  static double _kb = 300;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    widget.focus.addListener(_onFocus);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    widget.focus.removeListener(_onFocus);
    super.dispose();
  }

  @override
  void didChangeMetrics() {
    final view = View.of(context);
    final h = view.viewInsets.bottom / view.devicePixelRatio;
    if (h > 150) _kb = h;
  }

  void _onFocus() {
    // Maydonga bosildi — klaviatura chiqadi, panel yopiladi.
    if (widget.focus.hasFocus && _open) setState(() => _open = false);
  }

  void _toggle() {
    if (_open) {
      setState(() => _open = false);
      widget.focus.requestFocus();
    } else {
      widget.focus.unfocus();
      SystemChannels.textInput.invokeMethod('TextInput.hide');
      setState(() => _open = true);
    }
  }

  @override
  Widget build(BuildContext context) {
    final button = IconButton(
      onPressed: _toggle,
      visualDensity: VisualDensity.compact,
      icon: Icon(
        _open ? Icons.keyboard_outlined : Icons.emoji_emotions_outlined,
        color: Colors.white.withValues(alpha: 0.55),
        size: 24,
      ),
    );
    return PopScope(
      canPop: !_open,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop && _open) setState(() => _open = false);
      },
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          widget.row(context, button),
          AnimatedSize(
            duration: const Duration(milliseconds: 180),
            curve: Curves.easeOut,
            child: _open
                ? SizedBox(
                    height: _kb.clamp(240.0, 420.0),
                    child: TgMediaPanel(
                      controller: widget.controller,
                      onSticker: widget.onSticker,
                      onGif: widget.onGif,
                    ),
                  )
                : const SizedBox(width: double.infinity),
          ),
        ],
      ),
    );
  }
}

// ═══════════════════════════════════════════════════════════════
//  PANEL: EMOJI | GIF | STIKERLAR
// ═══════════════════════════════════════════════════════════════

class TgMediaPanel extends StatefulWidget {
  final TgTextController controller;
  final ValueChanged<TgDoc> onSticker;
  final ValueChanged<TgDoc> onGif;

  const TgMediaPanel({
    super.key,
    required this.controller,
    required this.onSticker,
    required this.onGif,
  });

  @override
  State<TgMediaPanel> createState() => _TgMediaPanelState();
}

class _TgMediaPanelState extends State<TgMediaPanel> {
  static int _lastTab = 0;
  late final PageController _pages = PageController(initialPage: _lastTab);
  int _tab = _lastTab;

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  void _go(int i) {
    setState(() => _tab = _lastTab = i);
    _pages.animateToPage(i,
        duration: const Duration(milliseconds: 250), curve: Curves.easeOut);
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      color: _Pal.bg,
      child: Stack(
        children: [
          PageView(
            controller: _pages,
            onPageChanged: (i) => setState(() => _tab = _lastTab = i),
            children: [
              _EmojiPage(controller: widget.controller),
              _GifPage(onGif: widget.onGif),
              _StickerPage(onSticker: widget.onSticker),
            ],
          ),
          // ── Pastda suzib turgan tugmalar (Telegram'dagidek) ──
          Positioned(
            left: 0,
            right: 0,
            bottom: 10,
            child: Center(
              child: Container(
                padding: const EdgeInsets.all(4),
                decoration: BoxDecoration(
                  color: const Color(0xEE2A2F36),
                  borderRadius: BorderRadius.circular(24),
                  boxShadow: const [
                    BoxShadow(color: Colors.black38, blurRadius: 10),
                  ],
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    for (final (i, label) in const [
                      (0, 'Emoji'),
                      (1, 'GIF'),
                      (2, 'Stikerlar'),
                    ])
                      GestureDetector(
                        onTap: () => _go(i),
                        child: AnimatedContainer(
                          duration: const Duration(milliseconds: 180),
                          padding: const EdgeInsets.symmetric(
                              horizontal: 16, vertical: 8),
                          decoration: BoxDecoration(
                            color: _tab == i
                                ? Colors.white.withValues(alpha: 0.12)
                                : Colors.transparent,
                            borderRadius: BorderRadius.circular(20),
                          ),
                          child: Text(
                            label,
                            style: TextStyle(
                              color: Colors.white
                                  .withValues(alpha: _tab == i ? 1 : 0.6),
                              fontSize: 14,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
          // ── ⌫ (faqat Emoji sahifasida) ──
          if (_tab == 0)
            Positioned(
              right: 10,
              bottom: 12,
              child: _RoundIcon(
                icon: Icons.backspace_outlined,
                onTap: widget.controller.backspace,
                onLong: widget.controller.backspace,
              ),
            ),
        ],
      ),
    );
  }
}

abstract final class _Pal {
  static const bg = Color(0xFF1B1F24);
  static const strip = Color(0xFF22272D);
  static const hint = Color(0xFF8A939D);
}

class _RoundIcon extends StatefulWidget {
  final IconData icon;
  final VoidCallback onTap;
  final VoidCallback? onLong;
  const _RoundIcon({required this.icon, required this.onTap, this.onLong});

  @override
  State<_RoundIcon> createState() => _RoundIconState();
}

class _RoundIconState extends State<_RoundIcon> {
  Timer? _repeat;

  @override
  void dispose() {
    _repeat?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: widget.onTap,
      // Bosib turilsa — ketma-ket o'chiradi.
      onLongPressStart: widget.onLong == null
          ? null
          : (_) => _repeat = Timer.periodic(
              const Duration(milliseconds: 70), (_) => widget.onLong!()),
      onLongPressEnd: (_) => _repeat?.cancel(),
      child: Container(
        width: 44,
        height: 38,
        decoration: BoxDecoration(
          color: const Color(0xEE2A2F36),
          borderRadius: BorderRadius.circular(19),
        ),
        child: Icon(widget.icon, color: Colors.white70, size: 20),
      ),
    );
  }
}

/// Bo'lim sarlavhasi.
class _Header extends StatelessWidget {
  final String title;
  final bool locked;
  const _Header(this.title, {this.locked = false});

  static const height = 34.0;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: height,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(14, 10, 14, 4),
        child: Row(
          children: [
            if (locked) ...[
              const Icon(Icons.lock_rounded, size: 14, color: _Pal.hint),
              const SizedBox(width: 4),
            ],
            Expanded(
              child: Text(
                title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                    color: _Pal.hint,
                    fontSize: 14,
                    fontWeight: FontWeight.w600),
              ),
            ),
            if (locked)
              const Text('Premium',
                  style: TextStyle(
                      color: Color(0xFFB57BFF),
                      fontSize: 12,
                      fontWeight: FontWeight.w700)),
          ],
        ),
      ),
    );
  }
}

/// Tepadagi bo'limlar qatori (belgi yoki rasm).
class _Strip extends StatelessWidget {
  final int count;
  final int selected;
  final Widget Function(int i) icon;
  final ValueChanged<int> onTap;
  const _Strip({
    required this.count,
    required this.selected,
    required this.icon,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 44,
      color: _Pal.strip,
      child: ListView.builder(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 6),
        itemCount: count,
        itemBuilder: (_, i) => GestureDetector(
          onTap: () => onTap(i),
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 150),
            width: 40,
            margin: const EdgeInsets.symmetric(horizontal: 2, vertical: 5),
            decoration: BoxDecoration(
              color: i == selected
                  ? Colors.white.withValues(alpha: 0.12)
                  : Colors.transparent,
              borderRadius: BorderRadius.circular(10),
            ),
            alignment: Alignment.center,
            child: icon(i),
          ),
        ),
      ),
    );
  }
}

/// Bo'limli uzun ro'yxat: har bir bo'limning balandligi OLDINDAN
/// ma'lum (sarlavha + qatorlar), shu sabab tepadagi qatordan bosilganda
/// o'sha joyga aniq sakraladi va aylantirilganda tanlangan belgi
/// o'zi almashadi.
class _Sections extends StatefulWidget {
  final int columns;
  final List<int> counts;
  final List<Widget> headers;
  final Widget Function(int section, int index, double cell) cell;

  /// Bo'lim hali yuklanmagan bo'lsa (to'plam) — butun bo'limni o'zi
  /// quradi.
  final Widget? Function(int section, double cell)? lazySection;
  final ValueChanged<int> onSection;
  final _SectionsJump jump;

  const _Sections({
    required this.columns,
    required this.counts,
    required this.headers,
    required this.cell,
    required this.onSection,
    required this.jump,
    this.lazySection,
  });

  @override
  State<_Sections> createState() => _SectionsState();
}

class _SectionsJump {
  void Function(int)? _to;
  void to(int section) => _to?.call(section);
}

class _SectionsState extends State<_Sections> {
  final _scroll = ScrollController();
  double _cellSize = 40;
  int _current = 0;

  @override
  void initState() {
    super.initState();
    widget.jump._to = _jumpTo;
    _scroll.addListener(_onScroll);
  }

  @override
  void didUpdateWidget(_Sections old) {
    super.didUpdateWidget(old);
    widget.jump._to = _jumpTo;
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  double _sectionHeight(int i) {
    final rows = (widget.counts[i] / widget.columns).ceil();
    return _Header.height + rows * _cellSize;
  }

  double _offsetOf(int section) {
    var y = 0.0;
    for (var i = 0; i < section; i++) {
      y += _sectionHeight(i);
    }
    return y;
  }

  void _jumpTo(int section) {
    if (!_scroll.hasClients) return;
    final y = _offsetOf(section).clamp(0.0, _scroll.position.maxScrollExtent);
    _scroll.animateTo(y,
        duration: const Duration(milliseconds: 280), curve: Curves.easeOut);
  }

  void _onScroll() {
    var y = 0.0;
    final at = _scroll.offset + 4;
    for (var i = 0; i < widget.counts.length; i++) {
      y += _sectionHeight(i);
      if (at < y) {
        if (i != _current) {
          _current = i;
          widget.onSection(i);
        }
        return;
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(builder: (context, box) {
      _cellSize = box.maxWidth / widget.columns;
      final cell = _cellSize;
      return CustomScrollView(
        controller: _scroll,
        slivers: [
          for (var s = 0; s < widget.counts.length; s++) ...[
            SliverToBoxAdapter(child: widget.headers[s]),
            if (widget.lazySection?.call(s, cell) case final lazy?)
              SliverToBoxAdapter(
                child: SizedBox(
                  height: (widget.counts[s] / widget.columns).ceil() * cell,
                  child: lazy,
                ),
              )
            else
              SliverGrid(
                gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                    crossAxisCount: widget.columns),
                delegate: SliverChildBuilderDelegate(
                  (_, i) => widget.cell(s, i, cell),
                  childCount: widget.counts[s],
                ),
              ),
          ],
          // Pastdagi suzuvchi tugmalar oxirgi qatorni yopmasin.
          const SliverToBoxAdapter(child: SizedBox(height: 60)),
        ],
      );
    });
  }
}

// ── EMOJI ────────────────────────────────────────────────────────

class _EmojiPage extends StatefulWidget {
  final TgTextController controller;
  const _EmojiPage({required this.controller});

  @override
  State<_EmojiPage> createState() => _EmojiPageState();
}

class _EmojiPageState extends State<_EmojiPage>
    with AutomaticKeepAliveClientMixin {
  final _jump = _SectionsJump();
  int _section = 0;
  List<String> _recent = [];
  List<TgDoc> _recentCustom = [];
  List<TgSet> _sets = [];
  bool _premium = false;

  static const _icons = [
    Icons.emoji_emotions_outlined,
    Icons.pets_outlined,
    Icons.fastfood_outlined,
    Icons.sports_soccer_outlined,
    Icons.directions_car_outlined,
    Icons.lightbulb_outline,
    Icons.emoji_symbols_outlined,
    Icons.flag_outlined,
  ];

  @override
  bool get wantKeepAlive => true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final m = TgMedia.instance;
    final r = await m.recentEmoji();
    final rc = await m.recentCustom();
    final premium = m.ready && await m.premium();
    final sets = m.ready ? await m.emojiSets() : <TgSet>[];
    if (!mounted) return;
    setState(() {
      _recent = r;
      _premium = premium;
      _recentCustom = premium ? rc : [];
      _sets = sets;
    });
  }

  void _say(String s) {
    ScaffoldMessenger.maybeOf(context)?.showSnackBar(SnackBar(
      behavior: SnackBarBehavior.floating,
      backgroundColor: AppColors.card,
      content: Text(s, style: const TextStyle(color: Colors.white)),
    ));
  }

  void _pickEmoji(String e) {
    widget.controller.insertText(e);
    unawaited(TgMedia.instance.noteEmoji(e));
  }

  void _pickCustom(TgDoc d) {
    if (!_premium) {
      _say('Maxsus emoji faqat Telegram Premium bilan yuboriladi');
      return;
    }
    TgMedia.instance.rememberEmoji(d);
    widget.controller.insertCustom(d);
    unawaited(TgMedia.instance.noteCustom(d));
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    final groups = tgEmojiGroups;
    // 0 — yaqinda, 1..8 — Unicode bo'limlari, keyin maxsus to'plamlar.
    final counts = <int>[
      _recent.length + _recentCustom.length,
      for (final g in groups) g.emoji.length,
      for (final s in _sets) s.count,
    ];
    final headers = <Widget>[
      const _Header('Yaqinda ishlatilgan'),
      for (final g in groups) _Header(g.title),
      for (final s in _sets) _Header(s.title, locked: !_premium),
    ];
    final firstSet = 1 + groups.length;
    return Column(
      children: [
        _Strip(
          count: 1 + groups.length + _sets.length,
          selected: _section,
          onTap: (i) {
            setState(() => _section = i);
            _jump.to(i);
          },
          icon: (i) {
            if (i == 0) {
              return const Icon(Icons.access_time, color: _Pal.hint, size: 22);
            }
            if (i <= groups.length) {
              return Icon(_icons[i - 1], color: _Pal.hint, size: 22);
            }
            final s = _sets[i - firstSet];
            return Stack(
              alignment: Alignment.center,
              children: [
                _SetIcon(set: s, size: 26),
                if (!_premium)
                  const Positioned(
                    right: 0,
                    bottom: 0,
                    child: Icon(Icons.lock_rounded,
                        size: 11, color: Colors.white70),
                  ),
              ],
            );
          },
        ),
        Expanded(
          child: _Sections(
            columns: 8,
            counts: counts,
            headers: headers,
            jump: _jump,
            onSection: (i) => setState(() => _section = i),
            cell: (s, i, cell) {
              if (s == 0) {
                if (i < _recentCustom.length) {
                  final d = _recentCustom[i];
                  return InkWell(
                    onTap: () => _pickCustom(d),
                    child: Center(
                        child: TgStickerView(doc: d, size: cell * 0.62)),
                  );
                }
                final e = _recent[i - _recentCustom.length];
                return _EmojiCell(e, cell, () => _pickEmoji(e));
              }
              final e = groups[s - 1].emoji[i];
              return _EmojiCell(e, cell, () => _pickEmoji(e));
            },
            lazySection: (s, cell) {
              if (s < firstSet) return null;
              final set = _sets[s - firstSet];
              return _SetGrid(
                set: set,
                columns: 8,
                cell: cell,
                scale: 0.62,
                locked: !_premium,
                onTap: _pickCustom,
              );
            },
          ),
        ),
      ],
    );
  }
}

class _EmojiCell extends StatelessWidget {
  final String e;
  final double cell;
  final VoidCallback onTap;
  const _EmojiCell(this.e, this.cell, this.onTap);

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Center(
        child: Text(e,
            style: TextStyle(fontSize: cell * 0.56, height: 1.1)),
      ),
    );
  }
}

/// To'plamning birinchi stikeri — tepadagi qator uchun belgi.
class _SetIcon extends StatelessWidget {
  final TgSet set;
  final double size;
  const _SetIcon({required this.set, required this.size});

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<List<TgDoc>>(
      future: TgMedia.instance.setDocs(set.id, set.hash),
      builder: (context, snap) {
        final docs = snap.data ?? const <TgDoc>[];
        if (docs.isEmpty) return SizedBox(width: size, height: size);
        final d = set.thumbDoc == null
            ? docs.first
            : docs.firstWhere((x) => x.id == set.thumbDoc,
                orElse: () => docs.first);
        return TgStickerView(doc: d, size: size, still: true);
      },
    );
  }
}

/// To'plam katakchalari (yuklanguncha bo'sh joy).
class _SetGrid extends StatelessWidget {
  final TgSet set;
  final int columns;
  final double cell;
  final double scale;
  final bool locked;
  final ValueChanged<TgDoc> onTap;

  const _SetGrid({
    required this.set,
    required this.columns,
    required this.cell,
    required this.scale,
    required this.onTap,
    this.locked = false,
  });

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<List<TgDoc>>(
      future: TgMedia.instance.setDocs(set.id, set.hash),
      builder: (context, snap) {
        final docs = snap.data ?? const <TgDoc>[];
        return Opacity(
          opacity: locked ? 0.55 : 1,
          child: Wrap(
            children: [
              for (final d in docs)
                SizedBox(
                  width: cell,
                  height: cell,
                  child: InkWell(
                    onTap: () => onTap(d),
                    borderRadius: BorderRadius.circular(8),
                    child: Center(
                      child: TgStickerView(
                          doc: d, size: cell * scale, still: true),
                    ),
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}

// ── STIKERLAR ────────────────────────────────────────────────────

class _StickerPage extends StatefulWidget {
  final ValueChanged<TgDoc> onSticker;
  const _StickerPage({required this.onSticker});

  @override
  State<_StickerPage> createState() => _StickerPageState();
}

class _StickerPageState extends State<_StickerPage>
    with AutomaticKeepAliveClientMixin {
  final _jump = _SectionsJump();
  int _section = 0;
  bool _loading = true;
  List<TgSet> _sets = [];
  List<TgDoc> _recent = [];
  List<TgDoc> _faved = [];

  @override
  bool get wantKeepAlive => true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    if (!TgMedia.instance.ready) {
      setState(() => _loading = false);
      return;
    }
    final r = await TgMedia.instance.stickers();
    if (!mounted) return;
    setState(() {
      _loading = false;
      _sets = r.sets;
      _recent = r.recent.take(20).toList();
      _faved = r.faved;
    });
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    if (_loading) {
      return const Center(
          child: CircularProgressIndicator(strokeWidth: 2, color: _Pal.hint));
    }
    if (_sets.isEmpty && _recent.isEmpty && _faved.isEmpty) {
      return const _Empty('Stikerlar topilmadi.\n'
          'Telegram\'da stiker to\'plamlarini qo\'shing — ular shu yerda chiqadi.');
    }
    // 0 — yaqinda, 1 — sevimlilar, keyin to'plamlar.
    final counts = [_recent.length, _faved.length, for (final s in _sets) s.count];
    final headers = <Widget>[
      const _Header('Yaqinda ishlatilgan'),
      const _Header('Sevimlilar'),
      for (final s in _sets) _Header(s.title),
    ];
    return Column(
      children: [
        _Strip(
          count: 2 + _sets.length,
          selected: _section,
          onTap: (i) {
            setState(() => _section = i);
            _jump.to(i);
          },
          icon: (i) => switch (i) {
            0 => const Icon(Icons.access_time, color: _Pal.hint, size: 22),
            1 => const Icon(Icons.star_border_rounded,
                color: _Pal.hint, size: 24),
            _ => _SetIcon(set: _sets[i - 2], size: 28),
          },
        ),
        Expanded(
          child: _Sections(
            columns: 5,
            counts: counts,
            headers: headers,
            jump: _jump,
            onSection: (i) => setState(() => _section = i),
            cell: (s, i, cell) {
              final d = s == 0 ? _recent[i] : _faved[i];
              return InkWell(
                onTap: () => widget.onSticker(d),
                borderRadius: BorderRadius.circular(8),
                child: Center(
                    child: TgStickerView(doc: d, size: cell * 0.86, still: true)),
              );
            },
            lazySection: (s, cell) => s < 2
                ? null
                : _SetGrid(
                    set: _sets[s - 2],
                    columns: 5,
                    cell: cell,
                    scale: 0.86,
                    onTap: widget.onSticker,
                  ),
          ),
        ),
      ],
    );
  }
}

class _Empty extends StatelessWidget {
  final String text;
  const _Empty(this.text);

  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(28, 0, 28, 50),
          child: Text(text,
              textAlign: TextAlign.center,
              style: const TextStyle(color: _Pal.hint, fontSize: 14, height: 1.4)),
        ),
      );
}

// ── GIF ──────────────────────────────────────────────────────────

class _GifPage extends StatefulWidget {
  final ValueChanged<TgDoc> onGif;
  const _GifPage({required this.onGif});

  @override
  State<_GifPage> createState() => _GifPageState();
}

class _GifPageState extends State<_GifPage>
    with AutomaticKeepAliveClientMixin {
  final _q = TextEditingController();
  final _scroll = ScrollController();
  Timer? _debounce;
  List<TgDoc> _saved = [];
  List<TgDoc> _found = [];
  String _next = '';
  String _query = '';
  bool _loading = true;
  bool _more = false;
  int _gen = 0;

  @override
  bool get wantKeepAlive => true;

  @override
  void initState() {
    super.initState();
    _scroll.addListener(() {
      if (_scroll.position.pixels > _scroll.position.maxScrollExtent - 300) {
        _loadMore();
      }
    });
    _start();
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _q.dispose();
    _scroll.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    if (!TgMedia.instance.ready) {
      setState(() => _loading = false);
      return;
    }
    final saved = await TgMedia.instance.savedGifs();
    if (!mounted) return;
    _saved = saved;
    await _search('');
  }

  /// Bo'sh so'rov — `@gif` mashhurlarni beradi (Telegram'dagidek).
  Future<void> _search(String q) async {
    final gen = ++_gen;
    setState(() {
      _query = q;
      _loading = true;
      _found = [];
      _next = '';
    });
    final r = await TgMedia.instance.searchGifs(q);
    if (!mounted || gen != _gen) return;
    setState(() {
      _loading = false;
      _found = r.docs;
      _next = r.next;
    });
  }

  Future<void> _loadMore() async {
    if (_more || _loading || _next.isEmpty) return;
    _more = true;
    final gen = _gen;
    final r = await TgMedia.instance.searchGifs(_query, offset: _next);
    _more = false;
    if (!mounted || gen != _gen) return;
    setState(() {
      _found.addAll(r.docs);
      _next = r.next;
    });
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    final items = [if (_query.isEmpty) ..._saved, ..._found];
    return Column(
      children: [
        Container(
          color: _Pal.strip,
          padding: const EdgeInsets.fromLTRB(10, 6, 10, 6),
          child: Container(
            height: 36,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            decoration: BoxDecoration(
              color: Colors.white.withValues(alpha: 0.07),
              borderRadius: BorderRadius.circular(18),
            ),
            child: Row(
              children: [
                const Icon(Icons.search, color: _Pal.hint, size: 20),
                const SizedBox(width: 6),
                Expanded(
                  child: TextField(
                    controller: _q,
                    style: const TextStyle(color: Colors.white, fontSize: 15),
                    cursorColor: Colors.white70,
                    decoration: const InputDecoration(
                      isDense: true,
                      border: InputBorder.none,
                      hintText: 'GIF qidirish',
                      hintStyle: TextStyle(color: _Pal.hint),
                    ),
                    onChanged: (v) {
                      _debounce?.cancel();
                      _debounce = Timer(const Duration(milliseconds: 400),
                          () => _search(v.trim()));
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
        Expanded(
          child: items.isEmpty
              ? (_loading
                  ? const Center(
                      child: CircularProgressIndicator(
                          strokeWidth: 2, color: _Pal.hint))
                  : const _Empty('GIF topilmadi'))
              : GridView.builder(
                  controller: _scroll,
                  padding: const EdgeInsets.fromLTRB(3, 3, 3, 64),
                  gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                    crossAxisCount: 3,
                    mainAxisSpacing: 3,
                    crossAxisSpacing: 3,
                    childAspectRatio: 1.15,
                  ),
                  itemCount: items.length,
                  itemBuilder: (_, i) => GestureDetector(
                    onTap: () => widget.onGif(items[i]),
                    child: ClipRRect(
                      borderRadius: BorderRadius.circular(6),
                      child: TgGifThumb(doc: items[i]),
                    ),
                  ),
                ),
        ),
      ],
    );
  }
}

/// Stiker yoki GIF xabari uchun o'lcham (Telegram'dagidek ~150).
double tgStickerSize(BuildContext context) =>
    math.min(150, MediaQuery.sizeOf(context).width * 0.4);
