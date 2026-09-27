// lib/widgets/tg_media_preview.dart — BOSIB TURGANDA KATTA KO'RINISH.
//
// TALAB (foydalanuvchi): "Telegram'da emoji, GIF va stikerni ustiga
// bosib turganda ekranning YUQORI qismida chiqib animatsiyalanadi —
// bizda shu ishlamayapti".
//
// Telegram `ContentPreviewViewer` kabi:
//   * bir marta bosish — darhol yuboradi (panelning o'zida);
//   * bosib turish — ekran tepasida katta stiker/emoji (tepasida
//     uning emojisi) yoki GIF chiqadi va shu yerda to'liq tezlikda
//     harakatlanadi; orqa fon xira qorayadi;
//   * barmoq qo'yib yuborilmasdan boshqa katakka surilsa — ko'rinish
//     o'sha katakka almashadi;
//   * barmoq qo'yib yuborilsa — yopiladi (hech narsa yuborilmaydi).
//
// Chat va izohlardagi stiker/GIF (u yerda ular TURADI — faqat birinchi
// kadr): bir marta bosilsa xuddi shu ko'rinish ochiladi
// ([TgHoldPreview.showTap]) va ekranning istalgan joyiga bosilsa
// yopiladi. Animatsiya faqat shu ko'rinishda bo'ladi.
//
// Orqa fon ATAYLAB xiralashtirilmaydi (blur): kuchsiz telefonda har
// kadrda butun ekranni xiralashtirish surishni qotirardi.

import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../services/tg_media.dart';
import 'tg_media_view.dart';

/// Ko'rsatilayotgan narsa: hujjat (panel, chatdagi stiker) yoki
/// diskdagi GIF fayli (chatdagi GIF, [ratio] — eni/bo'yi).
typedef _Shown = ({TgDoc? doc, String? path, bool gif, double ratio});

/// Bosib turilganda ko'rinadigan katta ko'rinish (bitta, butun ilova
/// uchun).
abstract final class TgHoldPreview {
  static OverlayEntry? _entry;
  static State? _owner;
  static final _shown = ValueNotifier<_Shown?>(null);

  static bool get open => _entry != null;

  static _Shown _of(TgDoc d, bool gif) => (
        doc: d,
        path: null,
        gif: gif,
        ratio: d.w > 0 && d.h > 0 ? d.w / d.h : 1.4,
      );

  static void show(BuildContext context, TgDoc doc, {required bool gif}) {
    HapticFeedback.selectionClick();
    _shown.value = _of(doc, gif);
    _insert(context, tapToClose: false);
  }

  /// Bir bosishda ochiladi, ekranning istalgan joyiga (yoki "orqaga")
  /// bosilsa yopiladi (chat va izohlardagi stiker [doc] yoki GIF fayli
  /// [path]). Alohida sahifa (route) — boshqa oynaga o'tilganda qolib
  /// ketmaydi.
  static void showTap(BuildContext context,
      {TgDoc? doc, String? path, double ratio = 1.4}) {
    if (doc == null && path == null) return;
    HapticFeedback.selectionClick();
    final shown = ValueNotifier<_Shown?>(doc != null
        ? _of(doc, doc.kind == 'mp4')
        : (doc: null, path: path, gif: true, ratio: ratio));
    Navigator.of(context, rootNavigator: true)
        .push(PageRouteBuilder<void>(
          opaque: false,
          transitionDuration: Duration.zero,
          reverseTransitionDuration: Duration.zero,
          pageBuilder: (_, __, ___) =>
              _PreviewLayer(shown: shown, tapToClose: true),
        ))
        .whenComplete(shown.dispose);
  }

  static void _insert(BuildContext context, {required bool tapToClose}) {
    if (_entry != null) return;
    final overlay = Overlay.maybeOf(context, rootOverlay: true);
    if (overlay == null) return;
    final e = OverlayEntry(
        builder: (_) => _PreviewLayer(shown: _shown, tapToClose: tapToClose));
    _entry = e;
    overlay.insert(e);
  }

  /// Barmoq [global] nuqtaga surildi — tagidagi katak ko'rsatiladi.
  static void moveTo(Offset global) {
    if (_entry == null) return;
    final t = TgHoldTarget._at(global);
    if (t == null) return;
    final cur = _shown.value;
    if (cur != null && cur.doc?.id == t.doc.id && cur.gif == t.gif) return;
    HapticFeedback.selectionClick();
    _shown.value = _of(t.doc, t.gif);
  }

  static void hide() {
    _owner = null;
    _entry?.remove();
    _entry = null;
    _shown.value = null;
  }
}

/// Panel katagi: bir marta bosilsa [onTap] (yuborish), bosib turilsa
/// katta ko'rinish.
class TgHoldTarget extends StatefulWidget {
  final TgDoc doc;
  final bool gif;
  final VoidCallback onTap;
  final Widget child;

  /// Bosilganda kichrayish darajasi (Telegram'dagidek).
  final double scale;

  const TgHoldTarget({
    super.key,
    required this.doc,
    required this.gif,
    required this.onTap,
    required this.child,
    this.scale = 0.86,
  });

  static final Set<_TgHoldTargetState> _all = {};

  static TgHoldTarget? _at(Offset global) {
    for (final s in _all) {
      final b = s.context.findRenderObject() as RenderBox?;
      if (b == null || !b.attached || !b.hasSize) continue;
      final local = b.globalToLocal(global);
      if ((Offset.zero & b.size).contains(local)) return s.widget;
    }
    return null;
  }

  @override
  State<TgHoldTarget> createState() => _TgHoldTargetState();
}

class _TgHoldTargetState extends State<TgHoldTarget> {
  bool _down = false;

  void _set(bool v) {
    if (_down != v) setState(() => _down = v);
  }

  @override
  void initState() {
    super.initState();
    TgHoldTarget._all.add(this);
  }

  @override
  void dispose() {
    TgHoldTarget._all.remove(this);
    // Bosib turilgan katak yo'qoldi (panel yopildi) — ko'rinish qolib
    // ketmasin.
    if (TgHoldPreview._owner == this) TgHoldPreview.hide();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTapDown: (_) => _set(true),
      onTapUp: (_) => _set(false),
      onTapCancel: () => _set(false),
      onTap: widget.onTap,
      onLongPressStart: (_) {
        _set(false);
        TgHoldPreview._owner = this;
        TgHoldPreview.show(context, widget.doc, gif: widget.gif);
      },
      onLongPressMoveUpdate: (d) => TgHoldPreview.moveTo(d.globalPosition),
      onLongPressEnd: (_) => TgHoldPreview.hide(),
      onLongPressCancel: TgHoldPreview.hide,
      child: AnimatedScale(
        scale: _down ? widget.scale : 1,
        duration: Duration(milliseconds: _down ? 90 : 220),
        curve: _down ? Curves.easeOut : Curves.easeOutBack,
        child: widget.child,
      ),
    );
  }
}

class _PreviewLayer extends StatefulWidget {
  final ValueNotifier<_Shown?> shown;

  /// Bir bosishda ochilgan — istalgan joyga bosilsa yopiladi (aks holda
  /// barmoq qo'yib yuborilganda yopiladi va bosishlarni o'tkazib yuboradi).
  final bool tapToClose;
  const _PreviewLayer({required this.shown, required this.tapToClose});

  @override
  State<_PreviewLayer> createState() => _PreviewLayerState();
}

class _PreviewLayerState extends State<_PreviewLayer>
    with SingleTickerProviderStateMixin {
  late final _a = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 180))
    ..forward();

  @override
  void dispose() {
    _a.dispose();
    super.dispose();
  }

  Widget _media(_Shown s, Size screen) {
    final side = math.min(screen.width, screen.height);
    final doc = s.doc;
    if (!s.gif && doc != null) {
      final size = math.min(side * 0.62, 300.0);
      return Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (doc.emoji.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: Text(doc.emoji,
                  style: const TextStyle(
                      fontSize: 30,
                      height: 1.2,
                      decoration: TextDecoration.none)),
            ),
          // Faqat shu yerda harakatlanadi.
          TgStickerView(
              key: ValueKey('p/${doc.id}'), doc: doc, size: size, live: true),
        ],
      );
    }
    final ratio = s.ratio;
    var w = screen.width - 48;
    var h = w / ratio;
    final maxH = screen.height * 0.4;
    if (h > maxH) {
      h = maxH;
      w = h * ratio;
    }
    return ClipRRect(
      borderRadius: BorderRadius.circular(12),
      child: SizedBox(
        width: w,
        height: h,
        child: doc != null
            ? TgGifThumb(key: ValueKey('p/${doc.id}'), doc: doc, loop: true)
            : TgAnimView(
                key: ValueKey('p/${s.path}'),
                path: s.path!,
                size: w,
                height: h,
                gif: true,
                live: true,
                fallback: const SizedBox()),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final screen = MediaQuery.sizeOf(context);
    final top = MediaQuery.paddingOf(context).top;
    final t = CurvedAnimation(parent: _a, curve: Curves.easeOutCubic);
    final layer = AnimatedBuilder(
      animation: t,
      builder: (context, child) => Stack(
        children: [
          Positioned.fill(
            child: ColoredBox(
                color: Colors.black.withValues(alpha: 0.6 * t.value)),
          ),
          Positioned(
            left: 0,
            right: 0,
            top: top + 36,
            child: Center(
              child: Opacity(
                opacity: t.value,
                child: Transform.scale(
                  scale: 0.8 + 0.2 * t.value,
                  alignment: Alignment.topCenter,
                  child: child,
                ),
              ),
            ),
          ),
        ],
      ),
      child: ValueListenableBuilder<_Shown?>(
        valueListenable: widget.shown,
        builder: (context, s, _) =>
            s == null ? const SizedBox.shrink() : _media(s, screen),
      ),
    );
    if (!widget.tapToClose) return IgnorePointer(child: layer);
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () => Navigator.of(context).maybePop(),
      child: layer,
    );
  }
}
