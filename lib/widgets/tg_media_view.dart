// lib/widgets/tg_media_view.dart — Telegram stikeri, maxsus emoji va
// GIF'ni chizish (`lib/services/tg_media.dart`).
//
//   * `webp` / rasm — oddiy rasm;
//   * `tgs` — Lottie animatsiya (gzip'langan JSON);
//   * `webm` (video stiker) — kichik rasmi (Android'dagi pleyer shaffof
//     fonli webm'ni chiza olmaydi);
//   * GIF — ovozsiz, takrorlanadigan mp4.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:lottie/lottie.dart';
import 'package:path_provider/path_provider.dart';
import 'package:video_player/video_player.dart';

import '../services/api_base.dart';
import '../services/telegram_service.dart';
import '../services/tg_media.dart';

/// Stiker yoki maxsus emoji.
class TgStickerView extends StatelessWidget {
  final TgDoc doc;
  final double size;

  /// Panelda: animatsiya faqat bir marta (ko'p stiker birdan
  /// aylanib, telefonni qizdirmasin).
  final bool still;

  const TgStickerView(
      {super.key, required this.doc, required this.size, this.still = false});

  @override
  Widget build(BuildContext context) {
    final thumbOnly = doc.kind == 'webm' || doc.kind == 'mp4';
    return SizedBox(
      width: size,
      height: size,
      child: FutureBuilder<String?>(
        future: TgMedia.instance.file(doc, thumb: thumbOnly && doc.thumb),
        builder: (context, snap) {
          final path = snap.data;
          if (path == null) return _Fallback(doc.emoji, size);
          if (doc.kind == 'tgs') return _Lottie(path, size, still);
          return Image.file(
            File(path),
            width: size,
            height: size,
            fit: BoxFit.contain,
            gaplessPlayback: true,
            errorBuilder: (_, __, ___) => _Fallback(doc.emoji, size),
          );
        },
      ),
    );
  }
}

class _Fallback extends StatelessWidget {
  final String emoji;
  final double size;
  const _Fallback(this.emoji, this.size);

  @override
  Widget build(BuildContext context) => Center(
        child: Text(emoji,
            style: TextStyle(fontSize: size * 0.6, height: 1)),
      );
}

class _Lottie extends StatefulWidget {
  final String path;
  final double size;
  final bool still;
  const _Lottie(this.path, this.size, this.still);

  @override
  State<_Lottie> createState() => _LottieState();
}

class _LottieState extends State<_Lottie> {
  static final Map<String, Future<LottieComposition?>> _cache = {};
  late Future<LottieComposition?> _comp;

  @override
  void initState() {
    super.initState();
    _comp = _cache[widget.path] ??= () async {
      try {
        return await LottieComposition.decodeGZip(
            await File(widget.path).readAsBytes());
      } catch (_) {
        return null;
      }
    }();
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<LottieComposition?>(
      future: _comp,
      builder: (context, snap) {
        final c = snap.data;
        if (c == null) return SizedBox(width: widget.size, height: widget.size);
        return Lottie(
          composition: c,
          width: widget.size,
          height: widget.size,
          repeat: !widget.still,
          fit: BoxFit.contain,
        );
      },
    );
  }
}

/// Xabardagi stiker (`stk_...` havolasi bo'yicha).
class TgStickerRefView extends StatelessWidget {
  final String ref;
  final double size;
  const TgStickerRefView({super.key, required this.ref, this.size = 150});

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<TgDoc?>(
      future: TgMedia.instance.stickerByRef(ref),
      builder: (context, snap) {
        final d = snap.data;
        if (d == null) {
          return SizedBox(
            width: size,
            height: size,
            child: snap.connectionState == ConnectionState.done
                ? Icon(Icons.image_not_supported_outlined,
                    color: Colors.white.withValues(alpha: 0.25))
                : null,
          );
        }
        return TgStickerView(doc: d, size: size);
      },
    );
  }
}

/// Matn ichidagi maxsus emoji (topilmaguncha oddiy emoji).
class TgCustomEmojiView extends StatelessWidget {
  final String id;
  final String alt;
  final double size;
  const TgCustomEmojiView(
      {super.key, required this.id, required this.alt, required this.size});

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<TgDoc?>(
      future: TgMedia.instance.customEmoji(id),
      builder: (context, snap) {
        final d = snap.data;
        if (d == null) return _Fallback(alt, size * 1.2);
        return TgStickerView(doc: d, size: size);
      },
    );
  }
}

/// Matnni maxsus emoji belgilari (`[ce:id:emoji]`) bilan bo'laklarga
/// ajratadi. Belgi bo'lmasa `null`.
List<InlineSpan>? customEmojiSpans(
    String text, TextStyle style, List<InlineSpan> Function(String) plain) {
  if (!text.contains('[ce:')) return null;
  final out = <InlineSpan>[];
  var last = 0;
  final size = (style.fontSize ?? 14) * 1.35;
  for (final m in customEmojiToken.allMatches(text)) {
    if (m.start > last) out.addAll(plain(text.substring(last, m.start)));
    out.add(WidgetSpan(
      alignment: PlaceholderAlignment.middle,
      child: TgCustomEmojiView(id: m.group(1)!, alt: m.group(2)!, size: size),
    ));
    last = m.end;
  }
  if (out.isEmpty) return null;
  if (last < text.length) out.addAll(plain(text.substring(last)));
  return out;
}

/// Maxsus emoji belgilarini oddiy emoji bilan almashtiradi
/// (bildirishnoma, ro'yxatdagi qisqa matn va nusxa olish uchun).
String plainEmojiText(String text) =>
    text.replaceAllMapped(customEmojiToken, (m) => m.group(2)!);

// ═══════════════════════════════════════════════════════════════
//  GIF
// ═══════════════════════════════════════════════════════════════

/// Panel katagi: GIF'ning kichik rasmi.
class TgGifThumb extends StatelessWidget {
  final TgDoc doc;
  const TgGifThumb({super.key, required this.doc});

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<String?>(
      future: doc.thumb ? TgMedia.instance.file(doc, thumb: true) : null,
      builder: (context, snap) {
        final p = snap.data;
        return Container(
          color: Colors.white.withValues(alpha: 0.05),
          child: p == null
              ? null
              : Image.file(File(p), fit: BoxFit.cover, gaplessPlayback: true),
        );
      },
    );
  }
}

/// Xabardagi GIF: kanaldagi fayl ([fileName]) — ovozsiz, takrorlanib.
class TgGifMessage extends StatefulWidget {
  final String fileName;
  final double maxWidth;
  const TgGifMessage(
      {super.key, required this.fileName, this.maxWidth = 230});

  @override
  State<TgGifMessage> createState() => _TgGifMessageState();
}

final Map<String, Future<File?>> _chatFiles = {};

/// Kanaldagi kichik fayl (GIF, dumaloq video) — bir marta olinadi va
/// vaqtinchalik papkada saqlanadi.
Future<File?> tgChatFile(String name) => _chatFiles[name] ??= () async {
        try {
          final dir = await getTemporaryDirectory();
          final f = File('${dir.path}/gif_$name');
          if (await f.exists() && await f.length() > 0) return f;
          final bytes = await TelegramService.instance
              .fetchBytes('$kApiBase/api/image/$name');
          if (bytes == null || bytes.isEmpty) {
            _chatFiles.remove(name);
            return null;
          }
          await f.writeAsBytes(bytes, flush: true);
          return f;
        } catch (_) {
          _chatFiles.remove(name);
          return null;
        }
      }();


class _TgGifMessageState extends State<TgGifMessage> {
  VideoPlayerController? _ctrl;
  bool _failed = false;

  @override
  void initState() {
    super.initState();
    _open();
  }

  Future<void> _open() async {
    final f = await tgChatFile(widget.fileName);
    if (!mounted) return;
    if (f == null) {
      setState(() => _failed = true);
      return;
    }
    final c = VideoPlayerController.file(f,
        videoPlayerOptions: VideoPlayerOptions(mixWithOthers: true));
    try {
      await c.initialize();
      await c.setLooping(true);
      await c.setVolume(0);
      await c.play();
    } catch (_) {
      await c.dispose();
      if (mounted) setState(() => _failed = true);
      return;
    }
    if (!mounted) {
      await c.dispose();
      return;
    }
    setState(() => _ctrl = c);
  }

  @override
  void dispose() {
    _ctrl?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = _ctrl;
    final ratio = c != null && c.value.isInitialized && c.value.aspectRatio > 0
        ? c.value.aspectRatio
        : 1.4;
    return ClipRRect(
      borderRadius: BorderRadius.circular(14),
      child: SizedBox(
        width: widget.maxWidth,
        height: widget.maxWidth / ratio,
        child: c == null
            ? Container(
                color: Colors.white.withValues(alpha: 0.06),
                alignment: Alignment.center,
                child: _failed
                    ? Icon(Icons.gif_box_outlined,
                        color: Colors.white.withValues(alpha: 0.3), size: 36)
                    : const SizedBox(
                        width: 22,
                        height: 22,
                        child: CircularProgressIndicator(
                            strokeWidth: 2, color: Colors.white54),
                      ),
              )
            : VideoPlayer(c),
      ),
    );
  }
}
