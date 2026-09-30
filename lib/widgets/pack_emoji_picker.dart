// lib/widgets/pack_emoji_picker.dart — ILOVANING O'Z EMOJI TANLASH OYNASI.
//
// Element (stiker, GIF, emoji) ga "mos emoji" tanlanganda telefonning
// klaviaturasi va uning emojilari EMAS, ilovaning Telegram emojilari
// (`TgEmoji` shrifti) chiqadi. Faqat kerakli bo'limlar ekranga chiziladi
// (`SliverGrid`), shu sabab kuchsiz telefonda ham silliq.

import 'package:flutter/material.dart';

import 'emoji_text.dart';
import 'glass.dart';
import 'tg_emoji_data.dart';

/// Emoji tanlash oynasini ochadi. Tanlangan emoji qaytadi; bo'sh satr —
/// "emojisiz"; oyna yopilsa `null`.
Future<String?> showPackEmojiPicker(BuildContext context) {
  return showModalBottomSheet<String>(
    context: context,
    isScrollControlled: true,
    backgroundColor: const Color(0xFF1C1C1E),
    shape: const RoundedRectangleBorder(
      borderRadius: BorderRadius.vertical(top: Radius.circular(22)),
    ),
    builder: (_) => const _PickerSheet(),
  );
}

class _PickerSheet extends StatelessWidget {
  const _PickerSheet();

  @override
  Widget build(BuildContext context) {
    final h = MediaQuery.sizeOf(context).height * 0.6;
    return SizedBox(
      height: h,
      child: Column(
        children: [
          const SizedBox(height: 8),
          Container(
            width: 38,
            height: 4,
            decoration: BoxDecoration(
              color: Colors.white24,
              borderRadius: BorderRadius.circular(2),
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 12, 8, 4),
            child: Row(
              children: [
                const Expanded(
                  child: Text('Mos emoji',
                      style: TextStyle(
                          color: Colors.white,
                          fontSize: 17,
                          fontWeight: FontWeight.w700)),
                ),
                TextButton(
                  onPressed: () => Navigator.of(context).pop(''),
                  child: const Text('Emojisiz',
                      style: TextStyle(color: AppColors.textDim)),
                ),
              ],
            ),
          ),
          Expanded(
            child: CustomScrollView(
              slivers: [
                for (final g in tgEmojiGroups) ...[
                  SliverToBoxAdapter(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(20, 12, 20, 6),
                      child: Text(g.title,
                          style: const TextStyle(
                              color: Color(0xFF8E8E93),
                              fontSize: 14,
                              fontWeight: FontWeight.w700)),
                    ),
                  ),
                  SliverPadding(
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    sliver: SliverGrid(
                      gridDelegate:
                          const SliverGridDelegateWithMaxCrossAxisExtent(
                        maxCrossAxisExtent: 46,
                      ),
                      delegate: SliverChildBuilderDelegate(
                        (ctx, i) {
                          final e = g.emoji[i];
                          return InkResponse(
                            onTap: () => Navigator.of(ctx).pop(e),
                            radius: 24,
                            child: Center(
                              child: Text(e,
                                  style: const TextStyle(
                                      fontFamily: kTgEmojiFont,
                                      fontSize: 28,
                                      height: 1.1)),
                            ),
                          );
                        },
                        childCount: g.emoji.length,
                        addAutomaticKeepAlives: false,
                      ),
                    ),
                  ),
                ],
                const SliverToBoxAdapter(child: SizedBox(height: 24)),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Tanlangan emojini Telegram shriftida chizadi.
class PackEmojiLabel extends StatelessWidget {
  final String emoji;
  final double size;
  const PackEmojiLabel(this.emoji, {super.key, this.size = 22});

  @override
  Widget build(BuildContext context) => Text(emoji,
      style: TextStyle(fontFamily: kTgEmojiFont, fontSize: size, height: 1.1));
}
