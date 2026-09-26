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
import 'emoji_text.dart';
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
      // Emoji — Telegram shriftida (`tgEmojiInputSpans`). Emoji
      // bo'lmasa odatdagi yo'l (klaviaturaning tagiga chizig'i bilan).
      final spans = tgEmojiInputSpans(text, style);
      if (spans == null) {
        return super.buildTextSpan(
            context: context, style: style, withComposing: withComposing);
      }
      return TextSpan(style: style, children: spans);
    }
    final size = (style?.fontSize ?? 14) * 1.3;
    final children = <InlineSpan>[];
    final buf = StringBuffer();
    void flush() {
      if (buf.isEmpty) return;
      final t = buf.toString();
      children.addAll(tgEmojiInputSpans(t, style) ?? [TextSpan(text: t)]);
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
//
// TALAB (foydalanuvchi): "emoji, gif, stiker oynasi Telegram'da
// qanday ko'rinsa xuddi shunday — bir tomchi suvdek bo'lsin, ui
// ko'rinishida ham ishlashida ham; tugmalarga ham Telegram'dagidek
// animatsiya".
//
// Telegram Android `EmojiView` tuzilishi:
//   * har sahifa tepasida bo'limlar qatori (emoji: 🕒 va turkumlar +
//     maxsus to'plamlar; stiker: ☆, 🕒 va to'plamlar), tanlangan
//     belgi ostida yumaloq "tabletka" SILJIB boradi;
//   * uning ostida "Qidiruv" qatori; GIF va stikerda ichida ❤️ 👍 👎
//     🎉 … turkum tugmalari (bosilsa shu emoji bo'yicha qidiradi);
//   * pastda suzib turgan "Emoji | GIF | Stikerlar", tanlangani ostida
//     tabletka siljiydi; emoji sahifasida o'ngda ⌫;
//   * katak bosilganda kichrayib-kattalashadi;
//   * GIF'lar balandligi bir xil qatorlarda, eni asl nisbatda (Telegram
//     `ExtendedGridLayoutManager`).

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
              TickerMode(
                  enabled: _tab == 1, child: _GifPage(onGif: widget.onGif)),
              TickerMode(
                  enabled: _tab == 2,
                  child: _StickerPage(onSticker: widget.onSticker)),
            ],
          ),
          // ── Yuklashda xato bo'lsa — sababi (bosilsa yopiladi) ──
          Positioned(
            left: 12,
            right: 12,
            bottom: 60,
            child: ValueListenableBuilder<String>(
              valueListenable: TgMedia.instance.lastError,
              builder: (context, err, _) => err.isEmpty
                  ? const SizedBox()
                  : GestureDetector(
                      onTap: () => TgMedia.instance.lastError.value = '',
                      child: Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 12, vertical: 8),
                        decoration: BoxDecoration(
                          color: const Color(0xEE3A1F22),
                          borderRadius: BorderRadius.circular(12),
                        ),
                        child: Text('Xato: $err',
                            maxLines: 3,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                                color: Color(0xFFFF8A8A), fontSize: 12)),
                      ),
                    ),
            ),
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
  static const field = Color(0x17FFFFFF);
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
  const _Press(
      {super.key, required this.child, this.onTap, this.scale = 0.86});

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
  final bool locked;
  const _Header(this.title, {this.locked = false});

  static const height = 36.0;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: height,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
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
                    fontSize: 15,
                    fontWeight: FontWeight.w500),
              ),
            ),
            if (locked)
              const Text('Premium',
                  style: TextStyle(
                      color: Color(0xFFB57BFF),
                      fontSize: 13,
                      fontWeight: FontWeight.w700)),
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

  static const item = 44.0;
  static const height = 48.0;

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
                left: widget.selected * _Strip.item + 3,
                top: 5,
                width: _Strip.item - 6,
                height: _Strip.item - 6,
                child: Container(
                  decoration: BoxDecoration(
                    color: _Pal.pillOn,
                    borderRadius: BorderRadius.circular(12),
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

/// Telegram'ning GIF va stiker qidiruvidagi turkum tugmalari.
const _searchChips = <(IconData, String)>[
  (Icons.favorite_border_rounded, '❤️'),
  (Icons.thumb_up_alt_outlined, '👍'),
  (Icons.thumb_down_alt_outlined, '👎'),
  (Icons.celebration_outlined, '🎉'),
  (Icons.sentiment_very_satisfied_outlined, '😂'),
  (Icons.sentiment_dissatisfied_outlined, '😢'),
  (Icons.sentiment_very_dissatisfied_outlined, '😡'),
  (Icons.waving_hand_outlined, '👋'),
  (Icons.bedtime_outlined, '😴'),
  (Icons.local_fire_department_outlined, '🔥'),
];

/// "Qidiruv" qatori. [chips] bo'lsa — o'ng tomonda turkum tugmalari.
class _SearchBar extends StatefulWidget {
  final ValueChanged<String> onQuery;
  final ValueChanged<String>? onChip;
  final String? chip;
  const _SearchBar({required this.onQuery, this.onChip, this.chip});

  @override
  State<_SearchBar> createState() => _SearchBarState();
}

class _SearchBarState extends State<_SearchBar> {
  final _c = TextEditingController();
  final _f = FocusNode();
  Timer? _debounce;

  @override
  void initState() {
    super.initState();
    _f.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _c.dispose();
    _f.dispose();
    super.dispose();
  }

  void _clear() {
    _c.clear();
    _f.unfocus();
    widget.onQuery('');
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final typing = _f.hasFocus || _c.text.isNotEmpty;
    return Padding(
      padding: const EdgeInsets.fromLTRB(10, 2, 10, 6),
      child: Container(
        height: 38,
        decoration: BoxDecoration(
          color: _Pal.field,
          borderRadius: BorderRadius.circular(19),
        ),
        child: Row(
          children: [
            const SizedBox(width: 10),
            AnimatedSwitcher(
              duration: const Duration(milliseconds: 180),
              child: widget.chip != null && !typing
                  ? _Press(
                      key: const ValueKey('back'),
                      onTap: () => widget.onChip?.call(''),
                      child: const Icon(Icons.arrow_back_rounded,
                          color: _Pal.icon, size: 22),
                    )
                  : const Icon(Icons.search_rounded,
                      key: ValueKey('search'), color: _Pal.icon, size: 22),
            ),
            const SizedBox(width: 6),
            Expanded(
              flex: typing || widget.onChip == null ? 10 : 4,
              child: TextField(
                controller: _c,
                focusNode: _f,
                style: const TextStyle(color: Colors.white, fontSize: 16),
                cursorColor: Colors.white70,
                textInputAction: TextInputAction.search,
                decoration: const InputDecoration(
                  isDense: true,
                  border: InputBorder.none,
                  hintText: 'Qidiruv',
                  hintStyle: TextStyle(color: _Pal.hint, fontSize: 16),
                ),
                onChanged: (v) {
                  setState(() {});
                  _debounce?.cancel();
                  _debounce = Timer(const Duration(milliseconds: 350),
                      () => widget.onQuery(v.trim()));
                },
              ),
            ),
            if (typing)
              _Press(
                onTap: _clear,
                child: const Padding(
                  padding: EdgeInsets.symmetric(horizontal: 10),
                  child: Icon(Icons.close_rounded, color: _Pal.icon, size: 20),
                ),
              )
            else if (widget.onChip != null)
              Expanded(
                flex: 7,
                child: ShaderMask(
                  shaderCallback: (r) => const LinearGradient(colors: [
                    Colors.transparent,
                    Colors.white,
                    Colors.white,
                  ], stops: [0, 0.08, 1])
                      .createShader(r),
                  blendMode: BlendMode.dstIn,
                  child: ListView(
                    scrollDirection: Axis.horizontal,
                    padding: const EdgeInsets.only(left: 6, right: 6),
                    children: [
                      for (final (icon, emoji) in _searchChips)
                        _Press(
                          onTap: () => widget.onChip!(emoji),
                          child: AnimatedContainer(
                            duration: const Duration(milliseconds: 180),
                            width: 36,
                            margin: const EdgeInsets.symmetric(vertical: 3),
                            decoration: BoxDecoration(
                              color: widget.chip == emoji
                                  ? _Pal.pillOn
                                  : Colors.transparent,
                              borderRadius: BorderRadius.circular(16),
                            ),
                            child: Icon(icon,
                                color: widget.chip == emoji
                                    ? Colors.white
                                    : _Pal.icon,
                                size: 22),
                          ),
                        ),
                    ],
                  ),
                ),
              ),
          ],
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

  /// Qidiruv qatori — ro'yxat bilan birga suriladi (Telegram'dagidek).
  final Widget? top;
  final double topHeight;

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
    this.top,
    this.topHeight = 0,
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
    var y = widget.topHeight;
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
    var y = widget.topHeight;
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
        slivers: [
          if (widget.top != null)
            SliverToBoxAdapter(
                child: SizedBox(height: widget.topHeight, child: widget.top)),
          for (var s = 0; s < widget.counts.length; s++)
            if (widget.counts[s] > 0) ...[
              SliverToBoxAdapter(child: widget.headers[s]),
              SliverPadding(
                padding: const EdgeInsets.symmetric(horizontal: 5),
                sliver: SliverGrid(
                  gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                      crossAxisCount: _columns),
                  delegate: SliverChildBuilderDelegate(
                    (_, i) => widget.cell(s, i, cell),
                    childCount: widget.counts[s],
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

/// Oddiy katakli ro'yxat (qidiruv natijalari).
class _Grid extends StatelessWidget {
  final double minCell;
  final int minColumns;
  final int count;
  final Widget Function(int i, double cell) cell;
  const _Grid({
    required this.minCell,
    required this.minColumns,
    required this.count,
    required this.cell,
  });

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(builder: (context, box) {
      final w = box.maxWidth - 10;
      final cols = math.max(minColumns, (w / minCell).floor());
      final size = w / cols;
      return GridView.builder(
        padding: const EdgeInsets.fromLTRB(5, 0, 5, 64),
        gridDelegate:
            SliverGridDelegateWithFixedCrossAxisCount(crossAxisCount: cols),
        itemCount: count,
        itemBuilder: (_, i) => cell(i, size),
      );
    });
  }
}

/// Bo'sh holat: sabab (xato bo'lsa — uning matni) va "Qayta urinish".
class _Empty extends StatelessWidget {
  final String text;
  final VoidCallback? onRetry;
  const _Empty(this.text, {this.onRetry});

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<String>(
      valueListenable: TgMedia.instance.lastError,
      builder: (context, err, _) => Center(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(28, 0, 28, 60),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(text,
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                      color: _Pal.hint, fontSize: 15, height: 1.4)),
              if (err.isNotEmpty && onRetry != null) ...[
                const SizedBox(height: 8),
                SelectableText('Xato: $err',
                    textAlign: TextAlign.center,
                    style: const TextStyle(
                        color: Color(0xFFFF6B6B), fontSize: 12)),
              ],
              if (onRetry != null) ...[
                const SizedBox(height: 12),
                _Press(
                  onTap: () {
                    TgMedia.instance.lastError.value = '';
                    onRetry!();
                  },
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                        horizontal: 18, vertical: 9),
                    decoration: BoxDecoration(
                      color: _Pal.pillOn,
                      borderRadius: BorderRadius.circular(18),
                    ),
                    child: const Text('Qayta urinish',
                        style: TextStyle(
                            color: Colors.white,
                            fontWeight: FontWeight.w600)),
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _Loading extends StatelessWidget {
  const _Loading();

  @override
  Widget build(BuildContext context) => const Center(
        child: Padding(
          padding: EdgeInsets.only(bottom: 50),
          child: SizedBox(
            width: 26,
            height: 26,
            child:
                CircularProgressIndicator(strokeWidth: 2.4, color: _Pal.hint),
          ),
        ),
      );
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

  String _query = '';
  List<String>? _found;

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

  Future<void> _search(String q) async {
    _query = q;
    if (q.isEmpty) {
      setState(() => _found = null);
      return;
    }
    final r = await TgMedia.instance.searchEmoji(q);
    if (!mounted || q != _query) return;
    setState(() => _found = r);
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
    final search = _SearchBar(onQuery: _search);
    final found = _found;
    return Column(
      children: [
        _Strip(
          count: 1 + groups.length + _sets.length,
          selected: _section,
          onTap: (i) {
            setState(() => _section = i);
            _jump.to(i);
          },
          icon: (i, on) {
            final c = on ? Colors.white : _Pal.icon;
            if (i == 0) return Icon(Icons.access_time, color: c, size: 22);
            if (i <= groups.length) {
              return Icon(_icons[i - 1], color: c, size: 22);
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
        if (found != null) search,
        Expanded(
          child: found != null
              ? (found.isEmpty
                  ? const _Empty('Hech narsa topilmadi')
                  : _Grid(
                      minCell: 45,
                      minColumns: 7,
                      count: found.length,
                      cell: (i, cell) => _EmojiCell(
                          found[i], cell, () => _pickEmoji(found[i])),
                    ))
              : _Sections(
                  minCell: 45,
                  minColumns: 7,
                  counts: counts,
                  headers: headers,
                  jump: _jump,
                  top: search,
                  topHeight: 46,
                  onSection: (i) => setState(() => _section = i),
                  cell: (s, i, cell) {
                    if (s >= firstSet) {
                      return _SetCell(
                        set: _sets[s - firstSet],
                        index: i,
                        size: cell * 0.7,
                        locked: !_premium,
                        onTap: _pickCustom,
                      );
                    }
                    if (s == 0) {
                      if (i < _recentCustom.length) {
                        final d = _recentCustom[i];
                        return _Press(
                          onTap: () => _pickCustom(d),
                          child: Center(
                              child: TgStickerView(
                                  doc: d, size: cell * 0.7, still: true)),
                        );
                      }
                      final e = _recent[i - _recentCustom.length];
                      return _EmojiCell(e, cell, () => _pickEmoji(e));
                    }
                    final e = groups[s - 1].emoji[i];
                    return _EmojiCell(e, cell, () => _pickEmoji(e));
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
    return _Press(
      onTap: onTap,
      scale: 0.8,
      child: Center(
        child: Text(e,
            style: TextStyle(
                fontFamily: kTgEmojiFont, fontSize: cell * 0.6, height: 1.1)),
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

/// To'plamning bitta katagi. To'plam ro'yxati bir marta olinadi
/// (`setDocs` keshlangan), katak esa faqat ekranga chiqqanda quriladi.
class _SetCell extends StatelessWidget {
  final TgSet set;
  final int index;
  final double size;
  final bool locked;
  final ValueChanged<TgDoc> onTap;

  const _SetCell({
    required this.set,
    required this.index,
    required this.size,
    required this.onTap,
    this.locked = false,
  });

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<List<TgDoc>>(
      future: TgMedia.instance.setDocs(set.id, set.hash),
      builder: (context, snap) {
        final docs = snap.data;
        if (docs == null || index >= docs.length) return const SizedBox();
        final d = docs[index];
        return _Press(
          onTap: () => onTap(d),
          child: Opacity(
            opacity: locked ? 0.6 : 1,
            child: Center(child: TgStickerView(doc: d, size: size, still: true)),
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

  /// Qidiruv: turkum tugmasi yoki yozilgan so'z bo'yicha natija.
  String? _chip;
  String _query = '';
  List<TgDoc>? _found;
  bool _searching = false;

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
    setState(() => _loading = true);
    final r = await TgMedia.instance.stickers(refresh: _sets.isEmpty);
    if (!mounted) return;
    setState(() {
      _loading = false;
      _sets = r.sets;
      _recent = r.recent.take(20).toList();
      _faved = r.faved;
    });
  }

  Future<void> _byEmoji(String emoji) async {
    if (emoji.isEmpty) {
      setState(() {
        _chip = null;
        _found = null;
      });
      return;
    }
    setState(() {
      _chip = emoji;
      _searching = true;
      _found = [];
    });
    final r = await TgMedia.instance.stickersByEmoji(emoji);
    if (!mounted || _chip != emoji) return;
    setState(() {
      _searching = false;
      _found = r;
    });
  }

  /// Yozilgan so'z: emoji kalit so'zlari orqali mos emoji topiladi va
  /// shu emoji'li stikerlar ko'rsatiladi (Telegram ham shunday qiladi).
  Future<void> _byText(String q) async {
    _query = q;
    if (q.isEmpty) return _byEmoji('');
    setState(() {
      _searching = true;
      _found = [];
    });
    final emoji = await TgMedia.instance.searchEmoji(q);
    final out = <TgDoc>[];
    final seen = <String>{};
    for (final e in emoji.take(3)) {
      for (final d in await TgMedia.instance.stickersByEmoji(e)) {
        if (seen.add(d.id)) out.add(d);
      }
    }
    if (!mounted || q != _query) return;
    setState(() {
      _chip = null;
      _searching = false;
      _found = out;
    });
  }

  Widget _cell(TgDoc d, double cell) => _Press(
        onTap: () => widget.onSticker(d),
        child: Center(
            child: TgStickerView(doc: d, size: cell * 0.86, still: true)),
      );

  @override
  Widget build(BuildContext context) {
    super.build(context);
    if (_loading) return const _Loading();
    final search = _SearchBar(onQuery: _byText, onChip: _byEmoji, chip: _chip);
    final found = _found;
    if (found != null) {
      return Column(
        children: [
          const SizedBox(height: 6),
          search,
          Expanded(
            child: found.isEmpty
                ? (_searching
                    ? const _Loading()
                    : const _Empty('Stikerlar topilmadi'))
                : _Grid(
                    minCell: 72,
                    minColumns: 5,
                    count: found.length,
                    cell: (i, cell) => _cell(found[i], cell),
                  ),
          ),
        ],
      );
    }
    if (_sets.isEmpty && _recent.isEmpty && _faved.isEmpty) {
      return _Empty(
          'Stikerlar topilmadi.\n'
          'Telegram\'da stiker to\'plamlarini qo\'shing — ular shu yerda chiqadi.',
          onRetry: _load);
    }
    // 0 — sevimlilar (☆), 1 — yaqinda (🕒), keyin to'plamlar.
    final counts = [_faved.length, _recent.length, for (final s in _sets) s.count];
    final headers = <Widget>[
      const _Header('Saralanganlar'),
      const _Header('Yaqinda ishlatilgan'),
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
          icon: (i, on) {
            final c = on ? Colors.white : _Pal.icon;
            return switch (i) {
              0 => Icon(Icons.star_border_rounded, color: c, size: 25),
              1 => Icon(Icons.access_time, color: c, size: 22),
              _ => _SetIcon(set: _sets[i - 2], size: 30),
            };
          },
        ),
        Expanded(
          child: _Sections(
            minCell: 72,
            minColumns: 5,
            counts: counts,
            headers: headers,
            jump: _jump,
            top: search,
            topHeight: 46,
            onSection: (i) => setState(() => _section = i),
            cell: (s, i, cell) {
              if (s >= 2) {
                return _SetCell(
                  set: _sets[s - 2],
                  index: i,
                  size: cell * 0.86,
                  onTap: widget.onSticker,
                );
              }
              return _cell(s == 0 ? _faved[i] : _recent[i], cell);
            },
          ),
        ),
      ],
    );
  }
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
  final _scroll = ScrollController();
  List<TgDoc> _saved = [];
  List<TgDoc> _found = [];
  String _next = '';
  String _query = '';
  String? _chip;
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
    _scroll.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    if (!TgMedia.instance.ready) {
      setState(() => _loading = false);
      return;
    }
    setState(() => _loading = true);
    final saved = await TgMedia.instance.savedGifs();
    if (!mounted) return;
    _saved = saved;
    await _search('');
  }

  /// Bo'sh so'rov — `@gif` mashhurlarni beradi (Telegram'dagidek).
  Future<void> _search(String q, {String? chip}) async {
    final gen = ++_gen;
    setState(() {
      _query = q;
      _chip = chip;
      _loading = true;
      _found = [];
      _next = '';
    });
    if (_scroll.hasClients) _scroll.jumpTo(0);
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
        const SizedBox(height: 6),
        _SearchBar(
          onQuery: (q) => _search(q),
          onChip: (e) => _search(e, chip: e.isEmpty ? null : e),
          chip: _chip,
        ),
        Expanded(
          child: items.isEmpty
              ? (_loading
                  ? const _Loading()
                  : _Empty('GIF topilmadi', onRetry: _start))
              : LayoutBuilder(
                  builder: (context, box) {
                    final rows = _justify(items, box.maxWidth, 110);
                    return ListView.builder(
                      controller: _scroll,
                      padding: const EdgeInsets.only(bottom: 64),
                      itemCount: rows.length,
                      itemBuilder: (_, r) {
                        final row = rows[r];
                        return Padding(
                          padding: const EdgeInsets.only(bottom: 2),
                          child: Row(
                            children: [
                              for (final (i, w) in row.items) ...[
                                if (i != row.items.first.$1)
                                  const SizedBox(width: 2),
                                SizedBox(
                                  width: w,
                                  height: row.height,
                                  child: _Press(
                                    scale: 0.92,
                                    onTap: () => widget.onGif(items[i]),
                                    child: TgGifThumb(doc: items[i]),
                                  ),
                                ),
                              ],
                            ],
                          ),
                        );
                      },
                    );
                  },
                ),
        ),
      ],
    );
  }
}

/// GIF qatori: indeks va eni, balandlik bir xil.
class _GifRow {
  final List<(int, double)> items;
  final double height;
  const _GifRow(this.items, this.height);
}

/// Telegram'dagidek: har qator [target] balandlikka yaqin, GIF'lar asl
/// nisbatida va qator butun kenglikni to'ldiradi.
List<_GifRow> _justify(List<TgDoc> docs, double width, double target) {
  const gap = 2.0;
  final rows = <_GifRow>[];
  var cur = <(int, double)>[];
  var sum = 0.0;
  void flush({bool last = false}) {
    if (cur.isEmpty) return;
    final gaps = gap * (cur.length - 1);
    var k = (width - gaps) / sum;
    // Oxirgi to'lmagan qator haddan tashqari cho'zilmasin.
    if (last && k * target > target * 1.3) k = 1.3;
    final h = target * k;
    rows.add(_GifRow([for (final (i, w) in cur) (i, w * k)], h));
    cur = [];
    sum = 0;
  }

  for (var i = 0; i < docs.length; i++) {
    final d = docs[i];
    final ratio = (d.w > 0 && d.h > 0) ? (d.w / d.h).clamp(0.5, 2.5) : 1.0;
    final w = target * ratio;
    cur.add((i, w));
    sum += w;
    if (sum + gap * (cur.length - 1) >= width) flush();
  }
  flush(last: true);
  return rows;
}

/// Stiker yoki GIF xabari uchun o'lcham (Telegram'dagidek ~150).
double tgStickerSize(BuildContext context) =>
    math.min(150, MediaQuery.sizeOf(context).width * 0.4);
