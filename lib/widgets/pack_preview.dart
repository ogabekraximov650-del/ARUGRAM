// lib/widgets/pack_preview.dart — BOSIB TURGANDA KATTA KO'RINISH VA MENYU.
//
// Telegram Android (`ContentPreviewViewer.java`) dagidek: katakni bosib
// turilsa orqa fon qorayadi (0x71000000, ~120 ms), element markazda
// KATTA ko'rinadi (o'lcham = ekranning kichik tomoni - 40 dp, burchak
// yumaloq), tepasida unga mos emoji (24 dp), pastida yumaloq (12) menyu
// (320 ms, easeOutQuint: tepadan 12 dp pastga siljib chiqadi).
// Animatsiyali element shu yerda o'ynaydi (bitta, ya'ni telefonga bosim yo'q).

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'emoji_text.dart';
import 'pack_views.dart';

class PackPreviewAction {
  final IconData icon;
  final String label;
  final VoidCallback onTap;
  final bool danger;
  const PackPreviewAction(this.icon, this.label, this.onTap,
      {this.danger = false});
}

/// Katta ko'rinish + menyu. Tanlangan amal oyna yopilgach bajariladi.
Future<void> showPackPreview(
  BuildContext context, {
  required int pack,
  required int item,
  required String emoji,
  required List<PackPreviewAction> actions,
  double aspect = 1,
}) {
  HapticFeedback.mediumImpact();
  return showGeneralDialog<void>(
    context: context,
    barrierDismissible: true,
    barrierLabel: 'Yopish',
    barrierColor: Colors.transparent,
    transitionDuration: const Duration(milliseconds: 120),
    pageBuilder: (ctx, _, __) => _PreviewBody(
      pack: pack,
      item: item,
      emoji: emoji,
      actions: actions,
      aspect: aspect,
    ),
    transitionBuilder: (ctx, anim, _, child) => FadeTransition(
      opacity: anim,
      child: child,
    ),
  );
}

class _PreviewBody extends StatelessWidget {
  final int pack;
  final int item;
  final String emoji;
  final List<PackPreviewAction> actions;
  final double aspect;
  const _PreviewBody({
    required this.pack,
    required this.item,
    required this.emoji,
    required this.actions,
    required this.aspect,
  });

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    // Telegram: min(eni, bo'yi) - 40 dp; juda katta bo'lib ketmasin.
    final side = (size.shortestSide - 40).clamp(160.0, 340.0);
    final w = aspect >= 1 ? side : side * aspect;
    final h = aspect >= 1 ? side / aspect : side;
    return Material(
      type: MaterialType.transparency,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => Navigator.of(context).pop(),
        child: Stack(
          fit: StackFit.expand,
          children: [
            BackdropFilter(
              filter: ui.ImageFilter.blur(sigmaX: 6, sigmaY: 6),
              child: const ColoredBox(color: Color(0xB3000000)),
            ),
            SafeArea(
              child: Center(
                child: SingleChildScrollView(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (emoji.isNotEmpty)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 12),
                          child: Text(emoji,
                              style: const TextStyle(
                                  fontFamily: kTgEmojiFont,
                                  fontSize: 30,
                                  height: 1.1)),
                        ),
                      ClipRRect(
                        borderRadius: BorderRadius.circular(14),
                        child: PackImage(
                          pack: pack,
                          item: item,
                          size: w,
                          height: h,
                          animate: true,
                          fit: BoxFit.contain,
                        ),
                      ),
                      const SizedBox(height: 24),
                      TweenAnimationBuilder<double>(
                        tween: Tween(begin: 0, end: 1),
                        duration: const Duration(milliseconds: 320),
                        curve: Curves.easeOutQuint,
                        builder: (_, t, child) => Opacity(
                          opacity: t,
                          child: Transform.translate(
                              offset: Offset(0, -12 * (1 - t)), child: child),
                        ),
                        child: _Menu(actions: actions),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Menu extends StatelessWidget {
  final List<PackPreviewAction> actions;
  const _Menu({required this.actions});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 260,
      padding: const EdgeInsets.symmetric(vertical: 6),
      decoration: BoxDecoration(
        color: const Color(0xF2352A24),
        borderRadius: BorderRadius.circular(14),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final a in actions)
            InkWell(
              onTap: () {
                Navigator.of(context).pop();
                a.onTap();
              },
              child: Padding(
                padding:
                    const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
                child: Row(
                  children: [
                    Icon(a.icon,
                        size: 22,
                        color: a.danger
                            ? const Color(0xFFE5606A)
                            : Colors.white70),
                    const SizedBox(width: 18),
                    Expanded(
                      child: Text(
                        a.label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: a.danger
                              ? const Color(0xFFE5606A)
                              : Colors.white.withValues(alpha: 0.92),
                          fontSize: 16.5,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}
