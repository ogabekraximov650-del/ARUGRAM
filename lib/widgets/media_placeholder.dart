// lib/widgets/media_placeholder.dart — eski stiker/GIF xabarlari.
//
// Telegram stiker, GIF va premium emoji tizimi olib tashlandi
// (foydalanuvchi talabi: ilova o'zining tizimini yasaydi). Ilgari
// yuborilgan stiker/GIF xabarlari Telegram'dan yuklanmaydi — o'rnida
// shu kichik belgi turadi.

import 'package:flutter/material.dart';

class MediaPlaceholder extends StatelessWidget {
  /// `sticker` yoki `gif`.
  final String type;
  const MediaPlaceholder({super.key, required this.type});

  @override
  Widget build(BuildContext context) {
    final gif = type == 'gif';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        color: const Color(0xF0241F1C),
        borderRadius: BorderRadius.circular(14),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(gif ? Icons.gif_box_outlined : Icons.emoji_emotions_outlined,
              size: 18, color: Colors.white70),
          const SizedBox(width: 6),
          Text(gif ? 'GIF' : 'Stiker',
              style: const TextStyle(color: Colors.white70, fontSize: 14)),
        ],
      ),
    );
  }
}
