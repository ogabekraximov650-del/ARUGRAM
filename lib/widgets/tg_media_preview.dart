// lib/widgets/tg_media_preview.dart — STIKER / GIF KO'RISH OYNASI.
//
// TALAB (foydalanuvchi): "Telegram'dagidek qil, lekin stiker va GIF'lar
// endi (panelda) animatsiyalanmasin. Faqat stiker yoki GIF ustiga bir
// marta bosganda yuborish tugmasi chiqsin va faqat shu joyda
// animatsiyalansin; ekranning bir chetiga bossa yo'qolsin".
//
// Telegram `ContentPreviewViewer` kabi:
//   * orqa fon xiralashadi va qorayadi;
//   * markazda katta stiker (tepasida uning emojisi) yoki GIF
//     (yumaloq burchakli) — shu yerda harakatlanadi;
//   * ostida menyu: "Stiker yuborish" / "GIF yuborish";
//   * oyna 0.8 dan kattalashib, xiralashib ochiladi (200 ms);
//   * bo'sh joyga bosilsa yopiladi.

import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../services/tg_media.dart';
import 'tg_media_view.dart';

/// Ko'rish oynasini ochadi. [onSend] — "yuborish" bosilganda.
Future<void> showTgMediaPreview(
  BuildContext context, {
  required TgDoc doc,
  required bool gif,
  required VoidCallback onSend,
}) {
  HapticFeedback.selectionClick();
  return Navigator.of(context, rootNavigator: true).push(PageRouteBuilder<void>(
    opaque: false,
    barrierDismissible: true,
    barrierLabel: 'close',
    transitionDuration: const Duration(milliseconds: 200),
    reverseTransitionDuration: const Duration(milliseconds: 160),
    pageBuilder: (context, a, _) =>
        _Preview(doc: doc, gif: gif, onSend: onSend, anim: a),
  ));
}

class _Preview extends StatelessWidget {
  final TgDoc doc;
  final bool gif;
  final VoidCallback onSend;
  final Animation<double> anim;

  const _Preview({
    required this.doc,
    required this.gif,
    required this.onSend,
    required this.anim,
  });

  Widget _media(BuildContext context, Size screen) {
    final side = math.min(screen.width, screen.height);
    if (!gif) {
      final size = math.min(side * 0.66, 320.0);
      return Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (doc.emoji.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: Text(doc.emoji,
                  style: const TextStyle(fontSize: 30, height: 1.2)),
            ),
          // Shu yerda harakatlanadi (panel emas — to'liq tezlikda).
          TgStickerView(doc: doc, size: size),
        ],
      );
    }
    final ratio = doc.w > 0 && doc.h > 0 ? doc.w / doc.h : 1.4;
    final maxW = screen.width - 48;
    final maxH = screen.height * 0.45;
    var w = maxW;
    var h = w / ratio;
    if (h > maxH) {
      h = maxH;
      w = h * ratio;
    }
    return ClipRRect(
      borderRadius: BorderRadius.circular(12),
      child: SizedBox(
        width: w,
        height: h,
        child: TgGifThumb(doc: doc, loop: true),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final screen = MediaQuery.sizeOf(context);
    final t = CurvedAnimation(parent: anim, curve: Curves.easeOutCubic);
    return AnimatedBuilder(
      animation: t,
      builder: (context, child) => Stack(
        children: [
          // Bo'sh joyga bosilsa — yopiladi.
          Positioned.fill(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: () => Navigator.of(context).pop(),
              child: BackdropFilter(
                filter: ui.ImageFilter.blur(
                    sigmaX: 14 * t.value, sigmaY: 14 * t.value),
                child: ColoredBox(
                    color: Colors.black.withValues(alpha: 0.55 * t.value)),
              ),
            ),
          ),
          Center(
            child: Opacity(
              opacity: t.value,
              child: Transform.scale(
                scale: 0.8 + 0.2 * t.value,
                child: child,
              ),
            ),
          ),
        ],
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          // Media ustiga bosish ham oynani yopmaydi.
          GestureDetector(onTap: () {}, child: _media(context, screen)),
          const SizedBox(height: 28),
          _Menu(
            label: gif ? 'GIF yuborish' : 'Stiker yuborish',
            onSend: () {
              Navigator.of(context).pop();
              onSend();
            },
          ),
        ],
      ),
    );
  }
}

/// `ActionBarPopupWindow`: qorong'i, 14 dp burchakli menyu.
class _Menu extends StatelessWidget {
  final String label;
  final VoidCallback onSend;
  const _Menu({required this.label, required this.onSend});

  @override
  Widget build(BuildContext context) {
    return Material(
      color: const Color(0xF22A2622),
      borderRadius: BorderRadius.circular(14),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onSend,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 14, 28, 14),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.send_outlined, color: Colors.white, size: 24),
              const SizedBox(width: 22),
              Text(label,
                  style: const TextStyle(
                      color: Colors.white,
                      fontSize: 16,
                      decoration: TextDecoration.none,
                      fontWeight: FontWeight.w400)),
            ],
          ),
        ),
      ),
    );
  }
}
