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
//   * GIF va Stikerlar, hamda emoji sahifasidagi maxsus emojilar —
//     ilovaning O'Z to'plamlaridan (`pack_service.dart`). Telegram'ning
//     premium emoji, GIF va stikerlari olib tashlangan (foydalanuvchi
//     talabi: kuchsiz telefonlar ko'tara olmadi).

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lottie/lottie.dart';

import 'package:path_provider/path_provider.dart';

import '../screens/my_packs_screen.dart';
import '../screens/pack_detail_screen.dart';
import '../services/auth_service.dart';
import '../services/pack_service.dart';
import 'pack_preview.dart';
import 'pack_views.dart';

import 'emoji_text.dart';
import 'tg_emoji_data.dart';

// ═══════════════════════════════════════════════════════════════
//  YOZISH MAYDONI BOSHQARUVCHISI
// ═══════════════════════════════════════════════════════════════

/// Yozish maydoni: emoji Telegram shriftida ko'rinadi.
class TgTextController extends TextEditingController {
  TgTextController({super.text});

  // ── ILOVANING O'Z TO'PLAMIDAGI EMOJI ────────────────────────
  //
  // Matnda BITTA belgi bo'lib turadi (shaxsiy foydalanish oralig'i,
  // U+E000...): kursor, belgilash va ⌫ oddiy belgidek ishlaydi. Yozish
  // maydonida rasm bo'lib chiziladi (`buildTextSpan`), yuborishda esa
  // `[pe:<to'plam>:<element>:<emoji>]` belgisiga aylanadi (`encoded`).
  static const int _puaBase = 0xE000;
  static const int _puaLimit = 900;
  final Map<int, PackPick> _packs = {};

  /// Bitta xabarda ko'pi bilan shuncha maxsus emoji (har biri yuborilganda
  /// ~25 belgi; server xabar uzunligini cheklaydi va belgi o'rtasidan
  /// kesilib qolmasligi kerak).
  static const int maxPerMessage = 20;

  bool get canAddPackEmoji =>
      text.runes.where(_packs.containsKey).length < maxPerMessage;

  void insertPackEmoji(PackPick p) {
    if (!canAddPackEmoji) return;
    int? code;
    _packs.forEach((c, v) {
      if (v.pack == p.pack && v.item == p.item) code = c;
    });
    if (code == null) {
      if (_packs.length >= _puaLimit) {
        // To'lgan: maydonda maxsus emoji qolmagan bo'lsa tozalanadi.
        if (text.runes.any(_packs.containsKey)) return;
        _packs.clear();
      }
      code = _puaBase + _packs.length;
      _packs[code!] = p;
    }
    insertText(String.fromCharCode(code!));
  }

  @override
  void clear() {
    _packs.clear();
    super.clear();
  }

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
  String get encoded {
    if (_packs.isEmpty) return text;
    final b = StringBuffer();
    for (final r in text.runes) {
      final p = _packs[r];
      b.write(p == null
          ? String.fromCharCode(r)
          : packEmojiToken(p.pack, p.item, p.emoji));
    }
    return b.toString();
  }

  bool get isBlank => text.trim().isEmpty;

  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    // Maxsus emoji (ilovaning o'z to'plamidan) — rasm bo'lib.
    if (_packs.isNotEmpty && text.runes.any(_packs.containsKey)) {
      final size = (style?.fontSize ?? 16) * 1.35;
      final out = <InlineSpan>[];
      final buf = StringBuffer();
      void flush() {
        if (buf.isEmpty) return;
        final t = buf.toString();
        out.add(TextSpan(children: tgEmojiInputSpans(t, style) ?? [TextSpan(text: t)]));
        buf.clear();
      }

      for (final r in text.runes) {
        final p = _packs[r];
        if (p == null) {
          buf.writeCharCode(r);
          continue;
        }
        flush();
        out.add(WidgetSpan(
          alignment: PlaceholderAlignment.middle,
          child: PackEmojiInline(
              pack: p.pack, item: p.item, emoji: p.emoji, size: size),
        ));
      }
      flush();
      return TextSpan(style: style, children: out);
    }
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

  /// Stiker yoki GIF tanlandi (xabar sifatida yuboriladi). Bo'lmasa —
  /// ular panelda ko'rinmaydi.
  final ValueChanged<PackPick>? onPickMedia;

  const TgInputArea({
    super.key,
    required this.controller,
    required this.focus,
    required this.row,
    this.onPickMedia,
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
                  child: TgMediaPanel(
                    controller: widget.controller,
                    onPickMedia: widget.onPickMedia,
                  ),
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
  final ValueChanged<PackPick>? onPickMedia;

  const TgMediaPanel({super.key, required this.controller, this.onPickMedia});

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
    return ClipRRect(
      // Telegram'dagidek: panelning yuqori burchaklari yumaloq.
      borderRadius: const BorderRadius.vertical(top: Radius.circular(22)),
      child: Container(
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
                  enabled: _tab == 1,
                  child: _PackPage(
                      kind: PackKind.gif, onPick: widget.onPickMedia)),
              TickerMode(
                  enabled: _tab == 2,
                  child: _PackPage(
                      kind: PackKind.sticker, onPick: widget.onPickMedia)),
            ],
          ),
          // ── Pastda suzib turgan tugmalar ──
          Positioned(
            left: 0,
            right: 0,
            bottom: 10,
            child: Center(child: _TabPill(tab: _tab, onTap: _go)),
          ),
          // ── ⚙ (GIF va Stikerlar sahifasida): to'plamlarni boshqarish ──
          Positioned(
            right: 10,
            bottom: 12,
            child: AnimatedScale(
              scale: _tab != 0 ? 1 : 0.4,
              duration: const Duration(milliseconds: 200),
              curve: Curves.easeOutBack,
              child: AnimatedOpacity(
                opacity: _tab != 0 ? 1 : 0,
                duration: const Duration(milliseconds: 160),
                child: IgnorePointer(
                  ignoring: _tab == 0,
                  child: _Press(
                    onTap: () => Navigator.of(context).push(
                      MaterialPageRoute<void>(
                        builder: (_) => MyPacksScreen(
                            initialKind:
                                _tab == 1 ? PackKind.gif : PackKind.sticker),
                      ),
                    ),
                    child: Container(
                      width: 44,
                      height: 44,
                      decoration: const BoxDecoration(
                        color: _Pal.pill,
                        shape: BoxShape.circle,
                        boxShadow: [
                          BoxShadow(color: Colors.black45, blurRadius: 12)
                        ],
                      ),
                      child: const Icon(Icons.settings_outlined,
                          color: Colors.white70, size: 22),
                    ),
                  ),
                ),
              ),
            ),
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
    ));
  }
}

abstract final class _Pal {
  static const bg = Color(0xFF201B18);
  static const pill = Color(0xF2352C27);
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

  /// Bosib turish (katta ko'rinish va menyu — `pack_preview.dart`).
  final VoidCallback? onLongPress;
  final double scale;
  const _Press(
      {required this.child, this.onTap, this.onLongPress, this.scale = 0.86});

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
      onLongPress: widget.onLongPress,
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

  /// Bosilsa — to'plam ochiladi (Telegram: to'plam nomiga bosish).
  final VoidCallback? onTap;
  const _Header(this.title, {this.onTap});

  /// `StickerSetNameCell`: balandligi 27 dp, nom 15, qalin.
  static const height = 30.0;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: onTap,
      child: SizedBox(
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
              if (onTap != null)
                const Icon(Icons.chevron_right_rounded,
                    color: _Pal.hint, size: 20),
            ],
          ),
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

  /// Tepadagi bo'limlar qatori (`EmojiTabsStrip`, 36-40 dp): pastga
  /// aylantirilganda tepaga chiqib yashirinadi, tepaga aylantirilganda
  /// qaytadi (Telegram: `checkTabsY`).
  final Widget? tabs;

  /// Ro'yxatning BIRINCHI elementi bo'lgan qidiruv qatori (50 dp,
  /// Telegram: `searchFieldHeight`) — u ham ro'yxat bilan aylanib ketadi.
  final Widget? leading;

  const _Sections({
    required this.minCell,
    required this.minColumns,
    required this.counts,
    required this.headers,
    required this.cell,
    required this.onSection,
    required this.jump,
    this.tabs,
    this.leading,
  });

  static const double searchHeight = 50;

  @override
  State<_Sections> createState() => _SectionsState();
}

class _SectionsJump {
  void Function(int)? _to;
  void to(int section) => _to?.call(section);
}

class _SectionsState extends State<_Sections> {
  final _scroll = ScrollController();
  final _tabsDy = ValueNotifier<double>(0);
  double _cellSize = 40;
  int _columns = 8;
  int _current = 0;
  bool _jumping = false;
  double _lastOffset = 0;

  double get _lead => widget.leading != null ? _Sections.searchHeight : 0;

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
    _tabsDy.dispose();
    super.dispose();
  }

  double _sectionHeight(int i) {
    final n = widget.counts[i];
    if (n == 0) return 0;
    final rows = (n / _columns).ceil();
    return _Header.height + rows * _cellSize;
  }

  double _offsetOf(int section) {
    var y = _lead;
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
    _tabsDy.value = 0;
    await _scroll.animateTo(y,
        duration: const Duration(milliseconds: 300),
        curve: Curves.easeOutCubic);
    _jumping = false;
    _lastOffset = _scroll.offset;
  }

  void _onScroll() {
    final off = _scroll.offset;
    // Tepadagi qator: pastga aylantirilsa yashirinadi, tepaga — qaytadi.
    if (widget.tabs != null && !_jumping) {
      final d = off - _lastOffset;
      _tabsDy.value = off <= 0
          ? 0
          : (_tabsDy.value - d).clamp(-_Strip.height, 0.0);
    }
    _lastOffset = off;
    if (_jumping) return;
    var y = _lead;
    final at = off + 4;
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
      return Stack(
        children: [
          CustomScrollView(
            controller: _scroll,
            // Faqat ekrandagi (va uning chetidagi bir qator) kataklar
            // quriladi va yuklanadi — ko'rinmagani yuklanmaydi.
            cacheExtent: cell,
            slivers: [
              // Tepadagi qator egallagan joy.
              if (widget.tabs != null)
                const SliverToBoxAdapter(child: SizedBox(height: _Strip.height)),
              if (widget.leading != null)
                SliverToBoxAdapter(
                  child: SizedBox(
                      height: _Sections.searchHeight, child: widget.leading),
                ),
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
              // Pastdagi suzuvchi tugmalar oxirgi qatorni yopmasin
              // (Telegram: pastdan 44 dp).
              const SliverToBoxAdapter(child: SizedBox(height: 64)),
            ],
          ),
          if (widget.tabs != null)
            Positioned(
              top: 0,
              left: 0,
              right: 0,
              child: ValueListenableBuilder<double>(
                valueListenable: _tabsDy,
                builder: (_, dy, child) =>
                    Transform.translate(offset: Offset(0, dy), child: child),
                child: ColoredBox(color: _Pal.bg, child: widget.tabs),
              ),
            ),
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

// ── QIDIRUV (Telegram'dagidek) ────────────────────────────────

String _baseEmoji(String e) => e.replaceAll('️', '');

/// Element qidiruvga mos keladimi: tezkor emoji (`chip`) elementning mos
/// emojisi bo'yicha, matn (`query`) — to'plam nomi yoki emoji bo'yicha.
bool _matches(PackInfo p, String emoji, String query, String chip) {
  if (chip.isNotEmpty && !_baseEmoji(emoji).contains(_baseEmoji(chip))) {
    return false;
  }
  if (query.isNotEmpty) {
    final q = query.toLowerCase();
    if (!p.title.toLowerCase().contains(q) && !emoji.contains(query)) {
      return false;
    }
  }
  return true;
}

/// "Qidiruv" + tezkor emoji tugmalari (❤ 👍 👎 ...).
class _PackSearch extends StatelessWidget {
  final String query;
  final String chip;
  final ValueChanged<String> onQuery;
  final ValueChanged<String> onChip;
  const _PackSearch({
    required this.query,
    required this.chip,
    required this.onQuery,
    required this.onChip,
  });

  static const chips = ['❤️', '👍', '👎', '🎉', '😊', '😢', '😂', '🔥', '🙏', '😡'];

  Future<void> _ask(BuildContext context) async {
    final ctl = TextEditingController(text: query);
    final r = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: const Color(0xFF2C2C2E),
        title: const Text('Qidiruv',
            style: TextStyle(color: Colors.white, fontSize: 17)),
        content: TextField(
          controller: ctl,
          autofocus: true,
          style: const TextStyle(color: Colors.white),
          decoration: const InputDecoration(
            hintText: 'To\'plam nomi yoki emoji',
            hintStyle: TextStyle(color: Colors.white38),
          ),
          onSubmitted: (v) => Navigator.of(ctx).pop(v),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(''),
              child: const Text('Tozalash')),
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(ctl.text),
              child: const Text('Qidirish')),
        ],
      ),
    );
    if (r != null) onQuery(r.trim());
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 42,
      margin: const EdgeInsets.fromLTRB(10, 4, 10, 4),
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.07),
        borderRadius: BorderRadius.circular(22),
      ),
      child: Row(
        children: [
          GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () => _ask(context),
            child: Padding(
              padding: const EdgeInsets.only(left: 14, right: 12),
              child: Row(
                children: [
                  const Icon(Icons.search_rounded, color: _Pal.hint, size: 22),
                  const SizedBox(width: 8),
                  ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 90),
                    child: Text(
                      query.isEmpty ? 'Qidiruv' : query,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                          color: query.isEmpty ? _Pal.hint : Colors.white,
                          fontSize: 16),
                    ),
                  ),
                ],
              ),
            ),
          ),
          Expanded(
            child: ListView(
              scrollDirection: Axis.horizontal,
              padding: const EdgeInsets.only(right: 8),
              children: [
                for (final c in chips)
                  _Press(
                    onTap: () => onChip(chip == c ? '' : c),
                    child: Container(
                      width: 40,
                      margin: const EdgeInsets.symmetric(vertical: 4, horizontal: 1),
                      decoration: BoxDecoration(
                        color: chip == c ? _Pal.pillOn : Colors.transparent,
                        borderRadius: BorderRadius.circular(16),
                      ),
                      child: Center(
                        child: Text(c,
                            style: const TextStyle(
                                fontFamily: kTgEmojiFont,
                                fontSize: 21,
                                height: 1.1)),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// GIF va Stikerlar oynasi: yaqinda ishlatilganlar + har bir to'plam
/// bo'limi (ilovaning O'Z to'plamlari — `pack_service.dart`).
///
/// Katakda faqat kichik STATIK rasm: o'nlab animatsiya bir vaqtda
/// ishlab kuchsiz telefonni qiynamasligi uchun. Tanlangan element xabarda
/// esa o'z animatsiyasi bilan chiqadi. Stikerlar — 4 ustunli katakcha,
/// GIF — Telegram'dagidek zich "devor" (har birining o'z nisbati).
class _PackPage extends StatefulWidget {
  final String kind;
  final ValueChanged<PackPick>? onPick;
  const _PackPage({required this.kind, this.onPick});

  @override
  State<_PackPage> createState() => _PackPageState();
}

class _PackPageState extends State<_PackPage>
    with AutomaticKeepAliveClientMixin {
  final _jump = _SectionsJump();
  final _wall = ScrollController();
  int _section = 0;
  final Map<int, PackHeader> _hdr = {};
  bool _first = true;
  String _query = '';
  String _chip = '';

  bool get _gif => widget.kind == PackKind.gif;
  bool get _filtering => _query.isNotEmpty || _chip.isNotEmpty;

  @override
  bool get wantKeepAlive => true;

  @override
  void initState() {
    super.initState();
    PackService.instance.addListener(_onSvc);
    unawaited(_init());
  }

  @override
  void dispose() {
    PackService.instance.removeListener(_onSvc);
    _wall.dispose();
    super.dispose();
  }

  Future<void> _init() async {
    await PackService.instance.load();
    await _loadHeaders();
    if (mounted) setState(() => _first = false);
  }

  void _onSvc() {
    if (!mounted) return;
    setState(() {});
    unawaited(_loadHeaders());
  }

  Future<void> _loadHeaders() async {
    final svc = PackService.instance;
    for (final p in svc.usable(widget.kind)) {
      final have = _hdr[p.id];
      if (have != null && have.ver >= p.version) continue;
      final h = await svc.header(p);
      if (!mounted) return;
      if (h != null) setState(() => _hdr[p.id] = h);
    }
  }

  List<PackItem> _items(PackInfo p) {
    final h = _hdr[p.id];
    if (h == null) return const [];
    final gone = PackService.instance.removedItems(p.id);
    return [
      for (final it in h.items)
        if (!gone.contains(it.id) && _matches(p, it.emoji, _query, _chip)) it,
    ];
  }

  void _openHub() {
    Navigator.of(context).push(MaterialPageRoute<void>(
        builder: (_) => MyPacksScreen(initialKind: widget.kind)));
  }

  void _openPack(PackInfo p) {
    Navigator.of(context).push(MaterialPageRoute<void>(
        builder: (_) => PackDetailScreen(packId: p.id, initial: p)));
  }

  void _pick(PackPick p) {
    HapticFeedback.selectionClick();
    PackService.instance.noteRecent(p);
    // Xabar chiqqanda o'zi darhol ko'rinsin.
    PackService.instance.prefetch(p.pack, p.item);
    widget.onPick?.call(p);
  }

  /// Muqova: to'plamning birinchi elementi, yumaloq burchakli.
  Widget _cover(PackInfo p, List<PackItem> l, bool on) {
    if (l.isEmpty) {
      return Icon(Icons.circle_outlined,
          color: on ? Colors.white : _Pal.icon, size: 18);
    }
    return Opacity(
      opacity: on ? 1 : 0.8,
      child: ClipRRect(
        borderRadius: BorderRadius.circular(8),
        child: PackImage(
            pack: p.id, item: l.first.id, size: 26, fit: BoxFit.cover),
      ),
    );
  }

  Widget _search() => _PackSearch(
        query: _query,
        chip: _chip,
        onQuery: (v) => setState(() => _query = v),
        onChip: (v) => setState(() => _chip = v),
      );

  PackInfo? _infoOf(int packId) {
    for (final p in PackService.instance.usable(widget.kind)) {
      if (p.id == packId) return p;
    }
    return null;
  }

  /// Bosib turish: katta ko'rinish + menyu (Telegram'dagidek).
  void _preview(PackPick pick, {double aspect = 1}) {
    final svc = PackService.instance;
    final info = _infoOf(pick.pack);
    final me = AuthService.instance.user?.id ?? 0;
    final fav = svc.isFavorite(pick);
    final what = _gif ? 'GIF' : 'Stiker';
    showPackPreview(
      context,
      pack: pick.pack,
      item: pick.item,
      emoji: pick.emoji,
      aspect: aspect,
      actions: [
        PackPreviewAction(Icons.send_rounded, '$what yuborish', () => _pick(pick)),
        PackPreviewAction(
          fav ? Icons.star_border_rounded : Icons.star_rounded,
          fav ? 'Saralanganlardan o\'chirish' : 'Saralanganlarga qo\'shish',
          () => svc.toggleFavorite(pick),
        ),
        if (info != null && me > 0 && info.ownerId == me)
          PackPreviewAction(
            Icons.delete_outline_rounded,
            'To\'plamdan o\'chirish',
            () => svc.removeItem(pick.pack, pick.item),
            danger: true,
          ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    final svc = PackService.instance;
    final packs = svc.usable(widget.kind);
    bool keep(PackPick r) =>
        _chip.isEmpty || _baseEmoji(r.emoji).contains(_baseEmoji(_chip));
    final recentAll = svc.recent(widget.kind);
    final favAll = svc.favorites(widget.kind);
    // Qidiruv matni bor bo'lsa yaqinda/saralangan bo'limlari yashiriladi
    // (ularda to'plam nomi yo'q).
    final favs = _query.isNotEmpty ? const <PackPick>[] : favAll.where(keep).toList();
    final recent =
        _query.isNotEmpty ? const <PackPick>[] : recentAll.where(keep).toList();
    final lists = [for (final p in packs) _items(p)];
    if (packs.isEmpty && recentAll.isEmpty && favAll.isEmpty) {
      return _PackEmpty(
          kind: widget.kind, loading: _first || svc.loading, onOpen: _openHub);
    }
    // 0 — saralanganlar (⭐), 1 — yaqinda (🕒), 2.. — to'plamlar.
    final counts = <int>[favs.length, recent.length, for (final l in lists) l.length];
    final empty = counts.every((c) => c == 0);
    if (_gif) {
      // GIF sahifasida tepadagi qator YO'Q (Telegram'dagidek): qidiruv va
      // zich "devor".
      return _gifWall(favs, recent, packs, lists, empty);
    }
    final sel = _section.clamp(0, counts.length - 1);
    final strip = _Strip(
      count: counts.length + 1,
      selected: sel,
      onTap: (i) {
        if (i >= counts.length) {
          _openHub();
          return;
        }
        setState(() => _section = i);
        _jump.to(i);
      },
      icon: (i, on) {
        final c = on ? Colors.white : _Pal.icon;
        if (i == 0) return Icon(Icons.star_border_rounded, color: c, size: 24);
        if (i == 1) return Icon(Icons.access_time_rounded, color: c, size: 22);
        if (i >= counts.length) {
          return Icon(Icons.add_circle_outline_rounded, color: c, size: 22);
        }
        return _cover(packs[i - 2], lists[i - 2], on);
      },
    );
    if (empty && _filtering) {
      return Column(children: [
        strip,
        _search(),
        const Expanded(
          child: Center(
              child: Text('Hech narsa topilmadi',
                  style: TextStyle(color: _Pal.hint, fontSize: 15))),
        ),
      ]);
    }
    return _stickerGrid(strip, favs, recent, packs, lists, counts);
  }

  Widget _stickerGrid(Widget strip, List<PackPick> favs, List<PackPick> recent,
      List<PackInfo> packs, List<List<PackItem>> lists, List<int> counts) {
    return _Sections(
      minCell: 72,
      minColumns: 4,
      counts: counts,
      tabs: strip,
      leading: _search(),
      headers: [
        const _Header('Saralanganlar'),
        const _Header('Yaqinda ishlatilgan'),
        for (final p in packs) _Header(p.title, onTap: () => _openPack(p)),
      ],
      jump: _jump,
      onSection: (i) => setState(() => _section = i),
      cell: (s, i, cell) {
        final PackPick pick;
        if (s == 0) {
          pick = favs[i];
        } else if (s == 1) {
          pick = recent[i];
        } else {
          final it = lists[s - 2][i];
          pick = PackPick(widget.kind, packs[s - 2].id, it.id, it.emoji);
        }
        return _Press(
          onTap: () => _pick(pick),
          onLongPress: () => _preview(pick),
          scale: 0.85,
          child: Padding(
            padding: const EdgeInsets.all(3),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(12),
              child: PackImage(
                  pack: pick.pack, item: pick.item, size: cell - 6, animate: true),
            ),
          ),
        );
      },
    );
  }

  /// GIF "devori": har qator kenglikka to'liq sig'adi, elementlar o'z nisbatida.
  /// Birinchi element — qidiruv qatori (u ham aylanib ketadi).
  Widget _gifWall(List<PackPick> favs, List<PackPick> recent,
      List<PackInfo> packs, List<List<PackItem>> lists, bool empty) {
    return LayoutBuilder(builder: (context, box) {
      const gap = 2.0;
      const target = 118.0;
      final w = box.maxWidth;
      final entries = <Object>[]; // _GifHead — sarlavha; _GifRow — qator

      void addSection(String title, List<_GifCell> cells, {PackInfo? pack}) {
        if (cells.isEmpty) return;
        entries.add(_GifHead(title, pack));
        var row = <_GifCell>[];
        var sum = 0.0;
        void flush(bool full) {
          if (row.isEmpty) return;
          final gaps = gap * (row.length - 1);
          final h = (full ? (w - gaps) / sum : target).clamp(60.0, 260.0);
          entries.add(_GifRow(row, h));
          row = <_GifCell>[];
          sum = 0;
        }

        for (final c in cells) {
          row.add(c);
          sum += c.aspect;
          if (sum * target + gap * (row.length - 1) >= w) flush(true);
        }
        flush(false);
      }

      double aspectOf(PackPick r) {
        for (final h in _hdr.values) {
          if (h.id != r.pack) continue;
          final it = h.find(r.item);
          if (it != null && it.w > 0 && it.h > 0) {
            return (it.w / it.h).clamp(0.5, 3.0);
          }
        }
        return 1.0;
      }

      addSection('Saralanganlar',
          [for (final r in favs) _GifCell(r, aspectOf(r), r.pack)]);
      addSection('Yaqinda ishlatilgan',
          [for (final r in recent) _GifCell(r, aspectOf(r), r.pack)]);
      for (var i = 0; i < packs.length; i++) {
        addSection(packs[i].title, [
          for (final it in lists[i])
            _GifCell(
              PackPick(widget.kind, packs[i].id, it.id, it.emoji),
              (it.w > 0 && it.h > 0) ? (it.w / it.h).clamp(0.5, 3.0) : 1.0,
              packs[i].id,
            ),
        ], pack: packs[i]);
      }
      return ListView.builder(
        controller: _wall,
        padding: const EdgeInsets.only(bottom: 70),
        itemCount: entries.length + 1,
        itemBuilder: (_, i) {
          if (i == 0) {
            return SizedBox(height: _Sections.searchHeight, child: _search());
          }
          final e = entries[i - 1];
          if (e is _GifHead) {
            final pk = e.pack;
            return _Header(e.title, onTap: pk == null ? null : () => _openPack(pk));
          }
          final r = e as _GifRow;
          final gaps = gap * (r.cells.length - 1);
          final sum = r.cells.fold<double>(0, (a, c) => a + c.aspect);
          final full = sum * target + gaps >= w;
          return Padding(
            padding: const EdgeInsets.only(bottom: gap),
            child: SizedBox(
              height: r.height,
              child: Row(
                children: [
                  for (var j = 0; j < r.cells.length; j++) ...[
                    if (j > 0) const SizedBox(width: gap),
                    SizedBox(
                      width: full
                          ? (w - gaps) * r.cells[j].aspect / sum
                          : r.height * r.cells[j].aspect,
                      height: r.height,
                      child: _Press(
                        onTap: () => _pick(r.cells[j].pick),
                        onLongPress: () =>
                            _preview(r.cells[j].pick, aspect: r.cells[j].aspect),
                        scale: 0.96,
                        child: PackImage(
                          pack: r.cells[j].pick.pack,
                          item: r.cells[j].pick.item,
                          size: full
                              ? (w - gaps) * r.cells[j].aspect / sum
                              : r.height * r.cells[j].aspect,
                          height: r.height,
                          animate: true,
                          fit: BoxFit.cover,
                        ),
                      ),
                    ),
                  ],
                ],
              ),
            ),
          );
        },
      );
    });
  }
}

class _GifHead {
  final String title;
  final PackInfo? pack;
  const _GifHead(this.title, this.pack);
}

class _GifCell {
  final PackPick pick;
  final double aspect;
  final int pack;
  const _GifCell(this.pick, this.aspect, this.pack);
}

class _GifRow {
  final List<_GifCell> cells;
  final double height;
  const _GifRow(this.cells, this.height);
}

/// To'plam yo'q bo'lgandagi oyna.
class _PackEmpty extends StatelessWidget {
  final String kind;
  final bool loading;
  final VoidCallback onOpen;
  const _PackEmpty(
      {required this.kind, required this.loading, required this.onOpen});

  @override
  Widget build(BuildContext context) {
    final gif = kind == PackKind.gif;
    return Center(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(28, 0, 28, 60),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(gif ? Icons.gif_box_outlined : Icons.emoji_emotions_outlined,
                size: 44, color: _Pal.hint),
            const SizedBox(height: 10),
            Text(PackKind.plural(kind),
                style: const TextStyle(
                    color: Colors.white,
                    fontSize: 16,
                    fontWeight: FontWeight.w600)),
            const SizedBox(height: 4),
            Text(
              loading
                  ? 'Yuklanmoqda...'
                  : "Hali to'plam yo'q. Ommaviy to'plamlardan qo'shing yoki "
                      "o'zingiz yarating.",
              textAlign: TextAlign.center,
              style: const TextStyle(color: _Pal.hint, fontSize: 14),
            ),
            if (!loading) ...[
              const SizedBox(height: 14),
              TextButton(
                onPressed: onOpen,
                child: const Text("To'plamlarni ochish"),
              ),
            ],
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
  final Map<int, PackHeader> _hdr = {};
  String _query = '';
  String _chip = '';

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
    PackService.instance.addListener(_onSvc);
    _load();
    unawaited(PackService.instance.load().then((_) => _loadHeaders()));
  }

  @override
  void dispose() {
    PackService.instance.removeListener(_onSvc);
    super.dispose();
  }

  void _onSvc() {
    if (!mounted) return;
    setState(() {});
    unawaited(_loadHeaders());
  }

  Future<void> _loadHeaders() async {
    final svc = PackService.instance;
    for (final p in svc.usable(PackKind.emoji)) {
      final have = _hdr[p.id];
      if (have != null && have.ver >= p.version) continue;
      final h = await svc.header(p);
      if (!mounted) return;
      if (h != null) setState(() => _hdr[p.id] = h);
    }
  }

  List<PackItem> _items(PackInfo p) {
    final h = _hdr[p.id];
    if (h == null) return const [];
    final gone = PackService.instance.removedItems(p.id);
    return [
      for (final it in h.items)
        if (!gone.contains(it.id) && _matches(p, it.emoji, _query, _chip)) it,
    ];
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

  void _pickPack(PackInfo p, PackItem it) {
    HapticFeedback.selectionClick();
    widget.controller.insertPackEmoji(PackPick(
        PackKind.emoji, p.id, it.id, it.emoji.isEmpty ? '🙂' : it.emoji));
  }

  /// Bosib turish: katta ko'rinish; "Emoji yuborish" va oddiy emojidan nusxa
  /// olish (Telegram'dagidek).
  void _previewPack(PackInfo p, PackItem it) {
    final plain = it.emoji.isEmpty ? '🙂' : it.emoji;
    showPackPreview(
      context,
      pack: p.id,
      item: it.id,
      emoji: plain,
      actions: [
        PackPreviewAction(Icons.send_rounded, 'Emoji yuborish', () => _pickPack(p, it)),
        PackPreviewAction(Icons.copy_rounded, 'Emojidan nusxa olish', () {
          Clipboard.setData(ClipboardData(text: plain));
        }),
      ],
    );
  }

  void _openHub() {
    Navigator.of(context).push(MaterialPageRoute<void>(
        builder: (_) => const MyPacksScreen(initialKind: PackKind.emoji)));
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    final groups = tgEmojiGroups;
    final packs = PackService.instance.usable(PackKind.emoji);
    final lists = [for (final p in packs) _items(p)];
    final ng = groups.length;
    // Telegram'dagidek tartib: 0 — yaqinda, 1..ng — Unicode bo'limlari,
    // keyin maxsus emoji to'plamlari.
    final counts = <int>[
      _recent.length,
      for (final g in groups) g.emoji.length,
      for (final l in lists) l.length,
    ];
    final headers = <Widget>[
      const _Header('Yaqinda ishlatilgan'),
      for (final g in groups) _Header(g.title),
      for (final p in packs)
        _Header(p.title,
            onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
                builder: (_) => PackDetailScreen(packId: p.id, initial: p)))),
    ];
    final strip = _Strip(
      count: counts.length + 1,
      selected: _section.clamp(0, counts.length - 1),
      onTap: (i) {
        if (i >= counts.length) {
          _openHub();
          return;
        }
        setState(() => _section = i);
        _jump.to(i);
      },
      icon: (i, on) {
        final c = on ? Colors.white : _Pal.icon;
        if (i == 0) return Icon(Icons.access_time_rounded, color: c, size: 22);
        if (i >= counts.length) {
          return Icon(Icons.add_circle_outline_rounded, color: c, size: 22);
        }
        if (i <= ng) {
          return _TabLottie(
              asset: 'assets/tg_anim/msg_emoji_${_icons[i - 1]}.json',
              selected: on,
              color: c);
        }
        final l = lists[i - 1 - ng];
        if (l.isEmpty) return Icon(Icons.circle_outlined, color: c, size: 18);
        return Opacity(
          opacity: on ? 1 : 0.75,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(6),
            child: PackImage(
                pack: packs[i - 1 - ng].id,
                item: l.first.id,
                size: 24,
                fit: BoxFit.cover),
          ),
        );
      },
    );
    return _Sections(
      minCell: 45,
      minColumns: 7,
      counts: counts,
      headers: headers,
      tabs: strip,
      leading: _PackSearch(
        query: _query,
        chip: _chip,
        onQuery: (v) => setState(() => _query = v),
        onChip: (v) => setState(() => _chip = v),
      ),
      jump: _jump,
      onSection: (i) => setState(() => _section = i),
      cell: (s, i, cell) {
        if (s == 0) return _EmojiCell(_recent[i], cell, _pickEmoji);
        if (s <= ng) return _EmojiCell(groups[s - 1].emoji[i], cell, _pickEmoji);
        final k = s - 1 - ng;
        final it = lists[k][i];
        return _Press(
          onTap: () => _pickPack(packs[k], it),
          onLongPress: () => _previewPack(packs[k], it),
          scale: 0.8,
          child: Padding(
            padding: EdgeInsets.all(cell * 0.14),
            child: PackImage(
                pack: packs[k].id, item: it.id, size: cell * 0.72, animate: true),
          ),
        );
      },
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
