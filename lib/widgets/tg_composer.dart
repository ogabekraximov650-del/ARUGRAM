// lib/widgets/tg_composer.dart — TELEGRAM'DAGIDEK YOZISH PANELI.
//
// TALAB (foydalanuvchi): "Telegram'ning pastki yozish va uchchala
// to'plamni ochadigan oynasi qanday ishlasa, xuddi shunday qilib
// yasab ber" (Emoji / GIF / Stikerlar); "premium emoji'ni
// ilovamiz obunasi bor odam yubora olsin (Telegram Premium shart emas)".
//
// ── QANDAY ISHLAYDI (Telegram Android bilan bir xil) ────────────
//
//   * Yozish maydonining chap tomonidagi 🙂 tugmasi klaviatura
//     o'rniga PANELNI ochadi (balandligi — klaviaturaniki), tugma
//     ⌨ ga aylanadi; yana bosilsa yoki maydonga bosilsa — klaviatura.
//   * Panelning pastida suzib turgan "Emoji | GIF | Stikerlar"
//     tugmalari, sahifalar yon tomonga suriladi.
//   * Emoji: tepada bo'limlar qatori (yaqinda, kulgichlar, hayvonlar
//     ... bayroqlar), bitta uzun ro'yxat; o'ng pastda ⌫ tugmasi.
//   * GIF va Stikerlar: hozircha bo'sh oynalar. Telegram'ning premium
//     emoji, GIF va stikerlari olib tashlandi (foydalanuvchi talabi:
//     "o'zimiz ilova uchun premium emoji, gif va stiker tizimni
//     yasaymiz").

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lottie/lottie.dart';

import 'package:path_provider/path_provider.dart';

import 'emoji_text.dart';
import 'tg_emoji_data.dart';

// ═══════════════════════════════════════════════════════════════
//  YOZISH MAYDONI BOSHQARUVCHISI
// ═══════════════════════════════════════════════════════════════

/// Yozish maydoni: emoji Telegram shriftida ko'rinadi.
class TgTextController extends TextEditingController {
  TgTextController({super.text});

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
  String get encoded => text;

  bool get isBlank => text.trim().isEmpty;

  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    // Emoji — Telegram shriftida (`tgEmojiInputSpans`). Emoji
    // bo'lmasa odatdagi yo'l (klaviaturaning tagiga chizig'i bilan).
    final spans = tgEmojiInputSpans(text, style);
    if (spans == null) {
      return super.buildTextSpan(
          context: context, style: style, withComposing: withComposing);
    }
    return TextSpan(style: style, children: spans);
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

  const TgInputArea({
    super.key,
    required this.controller,
    required this.focus,
    required this.row,
  });

  @override
  State<TgInputArea> createState() => _TgInputAreaState();
}

class _TgInputAreaState extends State<TgInputArea>
    with WidgetsBindingObserver, TickerProviderStateMixin {
  bool _open = false;

  /// Panel bir marta quriladi va keyin yopiq holda ham saqlanadi —
  /// qayta ochilishi darhol.
  bool _built = false;

  /// Oxirgi ko'rilgan klaviatura balandligi — panel xuddi shunday.
  static double _kb = 300;

  /// Panelning chiqishi (Telegram: 250 ms, `CubicBezierInterpolator.DEFAULT`).
  late final AnimationController _show = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 250));

  /// 🙂 <-> ⌨ belgisi (`smile_to_keyboard.json` / `keyboard_to_smile.json`).
  late final AnimationController _icon =
      AnimationController(vsync: this, value: 1);

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
    _show.dispose();
    _icon.dispose();
    super.dispose();
  }

  @override
  void didChangeMetrics() {
    final view = View.of(context);
    final h = view.viewInsets.bottom / view.devicePixelRatio;
    if (h > 150) _kb = h;
  }

  bool get _keyboardUp {
    final view = View.of(context);
    return view.viewInsets.bottom / view.devicePixelRatio > 100;
  }

  void _setOpen(bool v, {bool instant = false}) {
    if (v == _open) return;
    setState(() {
      _open = v;
      if (v) _built = true;
    });
    _icon.value = 0;
    if (_icon.duration != null) _icon.forward();
    if (instant) {
      _show.value = v ? 1 : 0;
    } else if (v) {
      _show.forward();
    } else {
      _show.reverse();
    }
  }

  void _onFocus() {
    // Maydonga bosildi — klaviatura chiqadi va panel O'RNIDA turadi
    // (bir zumda almashadi, pastga tushib-chiqmaydi).
    if (widget.focus.hasFocus && _open) _setOpen(false, instant: true);
  }

  void _toggle() {
    if (_open) {
      widget.focus.requestFocus();
      _setOpen(false, instant: true);
    } else {
      // Klaviatura ochiq bo'lsa — panel uning o'rnini darhol egallaydi;
      // yopiq bo'lsa — pastdan chiqadi.
      final instant = _keyboardUp;
      widget.focus.unfocus();
      SystemChannels.textInput.invokeMethod('TextInput.hide');
      _setOpen(true, instant: instant);
    }
  }

  @override
  Widget build(BuildContext context) {
    final button = GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: _toggle,
      child: SizedBox(
        width: 44,
        height: 44,
        child: Center(
          child: SizedBox(
            width: 26,
            height: 26,
            child: ColorFiltered(
              colorFilter: ColorFilter.mode(
                  Colors.white.withValues(alpha: 0.55), BlendMode.srcIn),
              child: Lottie.asset(
                _open
                    ? 'assets/tg_anim/smile_to_keyboard.json'
                    : 'assets/tg_anim/keyboard_to_smile.json',
                key: ValueKey(_open),
                controller: _icon,
                onLoaded: (c) {
                  _icon.duration = c.duration;
                  if (_icon.value < 1 && !_icon.isAnimating) _icon.forward();
                },
              ),
            ),
          ),
        ),
      ),
    );
    final h = _kb.clamp(240.0, 420.0);
    return PopScope(
      canPop: !_open,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop && _open) _setOpen(false);
      },
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          widget.row(context, button),
          if (_built)
            // Panel o'lchami O'ZGARMAYDI (qayta joylanmaydi) — faqat
            // ko'rinadigan qismi ochiladi: har kadrda yuzlab katakni
            // qayta joylash panelni qotirardi.
            AnimatedBuilder(
              animation: _show,
              builder: (context, child) {
                final t = Curves.easeOutCubic.transform(_show.value);
                // Yopiq panel daraxtdan CHIQMAYDI (holati saqlanadi,
                // qayta ochilishi darhol) — faqat ko'rinmaydi.
                if (t <= 0) return Offstage(child: child);
                return ClipRect(
                  child: Align(
                    alignment: Alignment.topCenter,
                    heightFactor: t,
                    child: child,
                  ),
                );
              },
              child: TickerMode(
                enabled: _open,
                child: SizedBox(
                  height: h,
                  child: TgMediaPanel(controller: widget.controller),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

// ═══════════════════════════════════════════════════════════════
//  PANEL: EMOJI | GIF | STIKERLAR
// ═══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): "emoji, gif, stiker oynasi Telegram'da
// qanday ko'rinsa xuddi shunday — bir tomchi suvdek bo'lsin, ui
// ko'rinishida ham ishlashida ham; tugmalarga ham Telegram'dagidek
// animatsiya".
//
// Telegram Android `EmojiView` tuzilishi:
//   * emoji sahifasi tepasida bo'limlar qatori (🕒 va turkumlar),
//     tanlangan belgi ostida yumaloq "tabletka" SILJIB boradi;
//   * pastda suzib turgan "Emoji | GIF | Stikerlar", tanlangani ostida
//     tabletka siljiydi; emoji sahifasida o'ngda ⌫;
//   * katak bosilganda kichrayib-kattalashadi.
//
// GIF va Stikerlar oynalari hozircha bo'sh (ilovaning o'z tizimi
// yasalguncha).

class TgMediaPanel extends StatefulWidget {
  final TgTextController controller;

  const TgMediaPanel({super.key, required this.controller});

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
    if (i == _tab) return;
    HapticFeedback.selectionClick();
    setState(() => _tab = _lastTab = i);
    _pages.animateToPage(i,
        duration: const Duration(milliseconds: 280),
        curve: Curves.easeOutCubic);
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
            // Ko'rinmayotgan sahifadagi animatsiyalar to'xtaydi.
            children: [
              TickerMode(
                  enabled: _tab == 0,
                  child: _EmojiPage(controller: widget.controller)),
              const _Soon(icon: Icons.gif_box_outlined, title: 'GIF'),
              const _Soon(
                  icon: Icons.emoji_emotions_outlined, title: 'Stikerlar'),
            ],
          ),
          // ── Pastda suzib turgan tugmalar ──
          Positioned(
            left: 0,
            right: 0,
            bottom: 10,
            child: Center(child: _TabPill(tab: _tab, onTap: _go)),
          ),
          // ── ⌫ (faqat Emoji sahifasida; paydo bo'lish animatsiyasi) ──
          Positioned(
            right: 10,
            bottom: 12,
            child: AnimatedScale(
              scale: _tab == 0 ? 1 : 0.4,
              duration: const Duration(milliseconds: 200),
              curve: Curves.easeOutBack,
              child: AnimatedOpacity(
                opacity: _tab == 0 ? 1 : 0,
                duration: const Duration(milliseconds: 160),
                child: IgnorePointer(
                  ignoring: _tab != 0,
                  child: _Backspace(onTap: widget.controller.backspace),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

abstract final class _Pal {
  static const bg = Color(0xFF1C1C1E);
  static const pill = Color(0xF22C2C2E);
  static const pillOn = Color(0x24FFFFFF);
  static const hint = Color(0xFF8E8E93);
  static const icon = Color(0xFF9A9AA0);
}

/// "Emoji | GIF | Stikerlar" — tanlangan yozuv ostida tabletka siljiydi.
class _TabPill extends StatelessWidget {
  final int tab;
  final ValueChanged<int> onTap;
  const _TabPill({required this.tab, required this.onTap});

  static const _labels = ['Emoji', 'GIF', 'Stikerlar'];
  static const _w = [72.0, 56.0, 92.0];

  @override
  Widget build(BuildContext context) {
    var left = 0.0;
    for (var i = 0; i < tab; i++) {
      left += _w[i];
    }
    return Container(
      padding: const EdgeInsets.all(4),
      decoration: BoxDecoration(
        color: _Pal.pill,
        borderRadius: BorderRadius.circular(24),
        boxShadow: const [BoxShadow(color: Colors.black45, blurRadius: 12)],
      ),
      child: SizedBox(
        height: 36,
        width: _w.reduce((a, b) => a + b),
        child: Stack(
          children: [
            AnimatedPositioned(
              duration: const Duration(milliseconds: 260),
              curve: Curves.easeOutCubic,
              left: left,
              top: 0,
              bottom: 0,
              width: _w[tab],
              child: Container(
                decoration: BoxDecoration(
                  color: _Pal.pillOn,
                  borderRadius: BorderRadius.circular(18),
                ),
              ),
            ),
            Row(
              children: [
                for (var i = 0; i < 3; i++)
                  _Press(
                    onTap: () => onTap(i),
                    child: SizedBox(
                      width: _w[i],
                      height: 36,
                      child: Center(
                        child: AnimatedDefaultTextStyle(
                          duration: const Duration(milliseconds: 200),
                          style: TextStyle(
                            color: Colors.white
                                .withValues(alpha: tab == i ? 1 : 0.55),
                            fontSize: 15,
                            fontWeight: FontWeight.w600,
                          ),
                          child: Text(_labels[i]),
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// Telegram'dagidek bosilganda kichrayadigan tugma.
class _Press extends StatefulWidget {
  final Widget child;
  final VoidCallback? onTap;
  final double scale;
  const _Press({required this.child, this.onTap, this.scale = 0.86});

  @override
  State<_Press> createState() => _PressState();
}

class _PressState extends State<_Press> {
  bool _down = false;

  void _set(bool v) {
    if (_down != v) setState(() => _down = v);
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTapDown: (_) => _set(true),
      onTapUp: (_) => _set(false),
      onTapCancel: () => _set(false),
      onTap: widget.onTap,
      child: AnimatedScale(
        scale: _down ? widget.scale : 1,
        duration: Duration(milliseconds: _down ? 90 : 220),
        curve: _down ? Curves.easeOut : Curves.easeOutBack,
        child: widget.child,
      ),
    );
  }
}

class _Backspace extends StatefulWidget {
  final VoidCallback onTap;
  const _Backspace({required this.onTap});

  @override
  State<_Backspace> createState() => _BackspaceState();
}

class _BackspaceState extends State<_Backspace> {
  Timer? _repeat;

  @override
  void dispose() {
    _repeat?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      // Bosib turilsa — ketma-ket o'chiradi (Telegram'dagidek tezlashib).
      onLongPressStart: (_) {
        var n = 0;
        _repeat = Timer.periodic(const Duration(milliseconds: 60), (_) {
          n++;
          widget.onTap();
          if (n > 12) widget.onTap();
        });
      },
      onLongPressEnd: (_) => _repeat?.cancel(),
      child: _Press(
        onTap: widget.onTap,
        child: Container(
          width: 44,
          height: 44,
          decoration: const BoxDecoration(
            color: _Pal.pill,
            shape: BoxShape.circle,
            boxShadow: [BoxShadow(color: Colors.black45, blurRadius: 12)],
          ),
          child: const Icon(Icons.backspace_outlined,
              color: Colors.white70, size: 20),
        ),
      ),
    );
  }
}

/// Bo'lim sarlavhasi.
class _Header extends StatelessWidget {
  final String title;
  const _Header(this.title);

  /// `StickerSetNameCell`: balandligi 27 dp, nom 15, qalin.
  static const height = 30.0;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: height,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(15, 9, 15, 0),
        child: Row(
          children: [
            Expanded(
              child: Text(
                title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                    color: _Pal.hint,
                    fontSize: 15,
                    fontWeight: FontWeight.w700),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Tepadagi bo'limlar qatori: tanlangan belgi ostida tabletka siljiydi,
/// tanlangani ko'rinmay qolsa qator o'zi suriladi.
class _Strip extends StatefulWidget {
  final int count;
  final int selected;
  final Widget Function(int i, bool on) icon;
  final ValueChanged<int> onTap;
  const _Strip({
    required this.count,
    required this.selected,
    required this.icon,
    required this.onTap,
  });

  // `EmojiTabsStrip`: tugma 30 dp, oraliq 3 dp, tanlov burchagi 8 dp.
  static const item = 33.0;
  static const height = 40.0;

  @override
  State<_Strip> createState() => _StripState();
}

class _StripState extends State<_Strip> {
  final _scroll = ScrollController();

  @override
  void didUpdateWidget(_Strip old) {
    super.didUpdateWidget(old);
    if (old.selected != widget.selected && _scroll.hasClients) {
      final x = 6 + widget.selected * _Strip.item;
      final view = _scroll.position.viewportDimension;
      final at = _scroll.offset;
      if (x < at || x + _Strip.item > at + view) {
        _scroll.animateTo(
            (x - view / 2 + _Strip.item / 2)
                .clamp(0.0, _scroll.position.maxScrollExtent),
            duration: const Duration(milliseconds: 250),
            curve: Curves.easeOut);
      }
    }
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: _Strip.height,
      child: SingleChildScrollView(
        controller: _scroll,
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 6),
        child: SizedBox(
          width: widget.count * _Strip.item,
          height: _Strip.height,
          child: Stack(
            children: [
              AnimatedPositioned(
                duration: const Duration(milliseconds: 250),
                curve: Curves.easeOutCubic,
                left: widget.selected * _Strip.item + 1.5,
                top: (_Strip.height - 30) / 2,
                width: 30,
                height: 30,
                child: Container(
                  decoration: BoxDecoration(
                    color: _Pal.pillOn,
                    borderRadius: BorderRadius.circular(8),
                  ),
                ),
              ),
              Row(
                children: [
                  for (var i = 0; i < widget.count; i++)
                    _Press(
                      onTap: () => widget.onTap(i),
                      child: SizedBox(
                        width: _Strip.item,
                        height: _Strip.height,
                        child: Center(
                            child: widget.icon(i, i == widget.selected)),
                      ),
                    ),
                ],
              ),
            ],
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
  /// Katakning eng kichik kengligi (Telegram: emoji — 45 dp, stiker —
  /// 72 dp); ustunlar soni kenglikdan hisoblanadi, lekin
  /// [minColumns] dan kam emas.
  final double minCell;
  final int minColumns;
  final List<int> counts;
  final List<Widget> headers;

  /// Katak. Faqat EKRANDA ko'ringanlari quriladi (`SliverGrid`).
  final Widget Function(int section, int index, double cell) cell;
  final ValueChanged<int> onSection;
  final _SectionsJump jump;

  const _Sections({
    required this.minCell,
    required this.minColumns,
    required this.counts,
    required this.headers,
    required this.cell,
    required this.onSection,
    required this.jump,
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
  int _columns = 8;
  int _current = 0;
  bool _jumping = false;

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
    final n = widget.counts[i];
    if (n == 0) return 0;
    final rows = (n / _columns).ceil();
    return _Header.height + rows * _cellSize;
  }

  double _offsetOf(int section) {
    var y = 0.0;
    for (var i = 0; i < section; i++) {
      y += _sectionHeight(i);
    }
    return y;
  }

  Future<void> _jumpTo(int section) async {
    if (!_scroll.hasClients) return;
    final y = _offsetOf(section).clamp(0.0, _scroll.position.maxScrollExtent);
    _current = section;
    _jumping = true;
    await _scroll.animateTo(y,
        duration: const Duration(milliseconds: 300),
        curve: Curves.easeOutCubic);
    _jumping = false;
  }

  void _onScroll() {
    if (_jumping) return;
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
      final w = box.maxWidth - 10;
      _columns = math.max(widget.minColumns, (w / widget.minCell).floor());
      _cellSize = w / _columns;
      final cell = _cellSize;
      return CustomScrollView(
        controller: _scroll,
        // Faqat ekrandagi (va uning chetidagi bir qator) kataklar
        // quriladi va yuklanadi — ko'rinmagani yuklanmaydi.
        cacheExtent: cell,
        slivers: [
          for (var s = 0; s < widget.counts.length; s++)
            if (widget.counts[s] > 0) ...[
              SliverToBoxAdapter(child: widget.headers[s]),
              SliverPadding(
                padding: const EdgeInsets.symmetric(horizontal: 5),
                sliver: SliverFixedExtentList(
                  itemExtent: cell,
                  delegate: SliverChildBuilderDelegate(
                    (_, r) => _gridRow(r, _columns, widget.counts[s], cell,
                        (i) => widget.cell(s, i, cell)),
                    childCount: (widget.counts[s] / _columns).ceil(),
                    addAutomaticKeepAlives: false,
                  ),
                ),
              ),
            ],
          // Pastdagi suzuvchi tugmalar oxirgi qatorni yopmasin.
          const SliverToBoxAdapter(child: SizedBox(height: 64)),
        ],
      );
    });
  }
}

/// Bitta qator: [cols] ta katak.
Widget _gridRow(int r, int cols, int count, double cell,
    Widget Function(int i) build) {
  return Row(
    children: [
      for (var j = 0; j < cols; j++)
        SizedBox(
          width: cell,
          height: cell,
          child: r * cols + j < count ? build(r * cols + j) : null,
        ),
    ],
  );
}

/// GIF / Stikerlar oynasi — ilovaning o'z tizimi yasalguncha bo'sh.
class _Soon extends StatelessWidget {
  final IconData icon;
  final String title;
  const _Soon({required this.icon, required this.title});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(28, 0, 28, 60),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 44, color: _Pal.hint),
            const SizedBox(height: 10),
            Text(title,
                style: const TextStyle(
                    color: Colors.white,
                    fontSize: 16,
                    fontWeight: FontWeight.w600)),
            const SizedBox(height: 4),
            const Text('Tez orada',
                textAlign: TextAlign.center,
                style: TextStyle(color: _Pal.hint, fontSize: 14)),
          ],
        ),
      ),
    );
  }
}

// ── EMOJI ────────────────────────────────────────────────────────

/// Yaqinda ishlatilgan oddiy emojilar (telefonda, ilova papkasida).
abstract final class _RecentEmoji {
  static const _max = 35;
  static List<String>? _list;

  static Future<File> _file() async {
    final dir = await getApplicationSupportDirectory();
    return File('${dir.path}/tg_recent_emoji.json');
  }

  static Future<List<String>> load() async {
    final cached = _list;
    if (cached != null) return List.of(cached);
    var out = <String>[];
    try {
      final j = jsonDecode(await (await _file()).readAsString());
      if (j is Map) {
        out = ((j['emoji'] as List?) ?? const []).whereType<String>().toList();
      }
    } catch (_) {}
    _list = out;
    return List.of(out);
  }

  static Future<void> note(String e) async {
    final l = _list ??= await load();
    l
      ..remove(e)
      ..insert(0, e);
    if (l.length > _max) l.removeLast();
    try {
      await (await _file()).writeAsString(jsonEncode({'emoji': l}));
    } catch (_) {}
  }
}

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

  /// Telegram `EmojiTabsStrip` belgilari — tanlanganda bir marta
  /// "jonlanadi" (`R.raw.msg_emoji_*`).
  static const _icons = [
    'smiles', 'cat', 'food', 'activities',
    'travel', 'objects', 'other', 'flags',
  ];

  @override
  bool get wantKeepAlive => true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final r = await _RecentEmoji.load();
    if (!mounted) return;
    setState(() => _recent = r);
  }

  void _pickEmoji(String e) {
    widget.controller.insertText(e);
    unawaited(_RecentEmoji.note(e));
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    final groups = tgEmojiGroups;
    // 0 — yaqinda, 1..8 — Unicode bo'limlari.
    final counts = <int>[
      _recent.length,
      for (final g in groups) g.emoji.length,
    ];
    final headers = <Widget>[
      const _Header('Yaqinda ishlatilgan'),
      for (final g in groups) _Header(g.title),
    ];
    return Column(
      children: [
        _Strip(
          count: 1 + groups.length,
          selected: _section,
          onTap: (i) {
            setState(() => _section = i);
            _jump.to(i);
          },
          icon: (i, on) {
            final c = on ? Colors.white : _Pal.icon;
            if (i == 0) return Icon(Icons.access_time_rounded, color: c, size: 22);
            return _TabLottie(
                asset: 'assets/tg_anim/msg_emoji_${_icons[i - 1]}.json',
                selected: on,
                color: c);
          },
        ),
        Expanded(
          child: _Sections(
            minCell: 45,
            minColumns: 7,
            counts: counts,
            headers: headers,
            jump: _jump,
            onSection: (i) => setState(() => _section = i),
            cell: (s, i, cell) => _EmojiCell(
                s == 0 ? _recent[i] : groups[s - 1].emoji[i], cell, _pickEmoji),
          ),
        ),
      ],
    );
  }
}

/// Emoji katagi (`ImageViewEmoji`): bosilganda 0.8 gacha kichrayadi;
/// teri rangi bor emojini BOSIB TURISH — rang tanlash oynasi
/// (`EmojiColorPickerWindow`: asl + 5 rang), tanlangani eslab qolinadi.
class _EmojiCell extends StatelessWidget {
  final String e;
  final double cell;
  final ValueChanged<String> onPick;
  const _EmojiCell(this.e, this.cell, this.onPick);

  /// Tanlangan ranglar (asl emoji -> rangli).
  static final Map<String, String> tones = {};

  static const _colors = ['', '🏻', '🏼', '🏽', '🏾', '🏿'];

  static String _base(String e) => e.replaceAll('\uFE0F', '');

  /// `EmojiView.addColorToCode`: rang oxirgi ZWJ bo'lagidan OLDIN
  /// qo'yiladi (masalan 🏃‍♂️ -> 🏃🏽‍♂).
  static String withColor(String code, String color) {
    if (color.isEmpty) return code;
    var c = _base(code);
    var end = '';
    var invert = false;
    if (c.endsWith('\u200D➡')) {
      c = c.substring(0, c.length - 2);
      invert = true;
    }
    final n = c.length;
    if (n > 2 && c[n - 2] == '\u200D') {
      end = c.substring(n - 2);
      c = c.substring(0, n - 2);
    } else if (n > 3 && c[n - 3] == '\u200D') {
      end = c.substring(n - 3);
      c = c.substring(0, n - 3);
    }
    return '$c$color$end${invert ? '\u200D➡' : ''}';
  }

  bool get _colorable => tgEmojiColored.contains(_base(e));

  void _openTones(BuildContext context) {
    final box = context.findRenderObject() as RenderBox?;
    final overlay = Overlay.of(context);
    if (box == null) return;
    HapticFeedback.mediumImpact();
    final at = box.localToGlobal(Offset.zero, ancestor: overlay.context.findRenderObject());
    late OverlayEntry entry;
    entry = OverlayEntry(
      builder: (_) => _TonePicker(
        emoji: e,
        anchor: at & box.size,
        cell: cell,
        onPick: (v) {
          entry.remove();
          if (v != null) onPick(v);
        },
      ),
    );
    overlay.insert(entry);
  }

  @override
  Widget build(BuildContext context) {
    final shown = tones[_base(e)] ?? e;
    return GestureDetector(
      onLongPress: _colorable ? () => _openTones(context) : null,
      child: _Press(
        onTap: () => onPick(shown),
        scale: 0.8,
        child: Center(
          child: Text(shown,
              style: TextStyle(
                  fontFamily: kTgEmojiFont, fontSize: cell * 0.6, height: 1.1)),
        ),
      ),
    );
  }
}

/// Teri rangi tanlash oynasi (katak ustida, 6 ta variant).
class _TonePicker extends StatefulWidget {
  final String emoji;
  final Rect anchor;
  final double cell;
  final ValueChanged<String?> onPick;

  const _TonePicker({
    required this.emoji,
    required this.anchor,
    required this.cell,
    required this.onPick,
  });

  @override
  State<_TonePicker> createState() => _TonePickerState();
}

class _TonePickerState extends State<_TonePicker>
    with SingleTickerProviderStateMixin {
  late final AnimationController _a = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 180))
    ..forward();

  @override
  void dispose() {
    _a.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    const n = 6;
    final item = widget.cell;
    final w = item * n + 8;
    final left = (widget.anchor.center.dx - w / 2).clamp(6.0, size.width - w - 6);
    final top = widget.anchor.top - item - 14;
    return Stack(
      children: [
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () => widget.onPick(null),
          ),
        ),
        Positioned(
          left: left,
          top: top,
          child: ScaleTransition(
            scale: CurvedAnimation(parent: _a, curve: Curves.easeOutBack),
            alignment: Alignment.bottomCenter,
            child: Material(
              color: _Pal.pill,
              elevation: 8,
              borderRadius: BorderRadius.circular(12),
              child: Padding(
                padding: const EdgeInsets.all(4),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    for (final c in _EmojiCell._colors)
                      _Press(
                        scale: 0.8,
                        onTap: () {
                          final v = _EmojiCell.withColor(widget.emoji, c);
                          final base = _EmojiCell._base(widget.emoji);
                          if (c.isEmpty) {
                            _EmojiCell.tones.remove(base);
                          } else {
                            _EmojiCell.tones[base] = v;
                          }
                          widget.onPick(v);
                        },
                        child: SizedBox(
                          width: item,
                          height: item,
                          child: Center(
                            child: Text(
                              _EmojiCell.withColor(widget.emoji, c),
                              style: TextStyle(
                                  fontFamily: kTgEmojiFont,
                                  fontSize: item * 0.6,
                                  height: 1.1),
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// Bo'lim belgisi (Lottie): tanlanganda bir marta o'ynaydi.
class _TabLottie extends StatefulWidget {
  final String asset;
  final bool selected;
  final Color color;
  const _TabLottie(
      {required this.asset, required this.selected, required this.color});

  @override
  State<_TabLottie> createState() => _TabLottieState();
}

class _TabLottieState extends State<_TabLottie>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(vsync: this);

  @override
  void didUpdateWidget(_TabLottie old) {
    super.didUpdateWidget(old);
    if (widget.selected && !old.selected && _c.duration != null) {
      _c.forward(from: 0);
    }
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 24,
      height: 24,
      child: ColorFiltered(
        colorFilter: ColorFilter.mode(widget.color, BlendMode.srcIn),
        child: Lottie.asset(
          widget.asset,
          controller: _c,
          onLoaded: (comp) {
            _c.duration = comp.duration;
            // Birinchi ko'rinishda oxirgi (tinch) kadr.
            if (!_c.isAnimating) _c.value = 1;
          },
        ),
      ),
    );
  }
}
