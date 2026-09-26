// lib/widgets/tg_media_view.dart — Telegram stikeri, maxsus emoji va
// GIF'ni chizish (`lib/services/tg_media.dart`).
//
//   * `webp` / rasm — oddiy rasm;
//   * `tgs` (Lottie) va `webm` (video stiker, shaffof fon bilan) —
//     Telegram'dagidek ilova ichidagi rlottie va libvpx bilan, fon
//     isolate'ida (`TgAnimView`);
//   * GIF — ovozsiz, takrorlanadigan mp4.

import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:path_provider/path_provider.dart';
import 'package:video_player/video_player.dart';

import '../services/api_base.dart';
import '../services/native_pool.dart';
import '../services/telegram_service.dart';
import '../services/tg_media.dart';

/// Stiker yoki maxsus emoji.
class TgStickerView extends StatelessWidget {
  final TgDoc doc;
  final double size;

  /// Panelda (Telegram'dagidek): kichikroq o'lchamda, kadrlar
  /// xotirada saqlanib, cheklangan sonda harakatlanadi.
  final bool still;

  const TgStickerView(
      {super.key, required this.doc, required this.size, this.still = false});

  @override
  Widget build(BuildContext context) {
    final animated = doc.kind == 'tgs' || doc.kind == 'webm';
    return SizedBox(
      width: size,
      height: size,
      child: FutureBuilder<String?>(
        future: TgMedia.instance.file(doc, thumb: doc.kind == 'mp4' && doc.thumb),
        builder: (context, snap) {
          final path = snap.data;
          if (path == null) return _Fallback(doc.emoji, size);
          if (animated) {
            return TgAnimView(
              path: path,
              size: size,
              panel: still,
              fallback: _Fallback(doc.emoji, size),
            );
          }
          final px = (size * MediaQuery.devicePixelRatioOf(context)).round();
          return Image.file(
            File(path),
            width: size,
            height: size,
            // Asl 512px emas — ko'rinadigan o'lchamda ochiladi.
            cacheWidth: px,
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

// ═══════════════════════════════════════════════════════════════
//  ANIMATSIYA (rlottie / libvpx, Rust yadrosida — `sticker_anim.rs`)
// ═══════════════════════════════════════════════════════════════
//
// Telegram/Cherrygram (`RLottieDrawable`) kabi:
//   * kadr FON isolate'ida chiziladi (`NativePool.render`), UI oqimi
//     faqat tayyor rasmni ko'rsatadi — hech narsa qotmaydi;
//   * bitta animatsiyaga bir vaqtda faqat BITTA so'rov: kadr
//     ulgurmasa tashlab o'tiladi (ekran to'xtab qolmaydi);
//   * birinchi aylanishda chizilgan kadrlar xotirada qoladi (hajmi
//     kichik bo'lsa) — keyingi aylanishlar protsessorni ishlatmaydi;
//   * panelda o'lcham kichikroq va bir vaqtda eng ko'pi
//     [_panelSlots] ta stiker harakatlanadi, qolganlari birinchi kadrda
//     turadi (joy bo'shashi bilan ular ham harakatlanadi).

/// Birinchi kadrlar keshi — panel qayta ochilganda darhol ko'rinsin.
final Map<String, ui.Image> _firstFrames = {};
const _firstFramesMax = 300;

/// Panelda bir vaqtda harakatlanadigan stikerlar.
const _panelSlots = 18;
int _panelBusy = 0;
final List<VoidCallback> _panelWaiters = [];

class TgAnimView extends StatefulWidget {
  final String path;
  final double size;
  final bool panel;
  final Widget fallback;

  const TgAnimView({
    super.key,
    required this.path,
    required this.size,
    required this.fallback,
    this.panel = false,
  });

  @override
  State<TgAnimView> createState() => _TgAnimViewState();
}

class _TgAnimViewState extends State<TgAnimView>
    with SingleTickerProviderStateMixin {
  Ticker? _ticker;
  int _handle = 0;
  int _frames = 1;
  double _fps = 30;
  int _px = 0;
  ui.Image? _image;
  bool _busy = false;
  bool _dead = false;
  bool _slot = false;
  int _shown = -1;

  /// Xotiradagi kadrlar (sig'sa).
  List<ui.Image?>? _cache;

  String get _key => '${widget.path}@$_px';

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_px != 0) return;
    final dpr = MediaQuery.devicePixelRatioOf(context);
    _px = (widget.size * dpr)
        .round()
        .clamp(24, widget.panel ? 160 : 384)
        .toInt();
    final first = _firstFrames[_key];
    if (first != null) _image = first.clone();
    _open();
  }

  Future<void> _open() async {
    final r = await NativePool.render.animOpen(widget.path, _px, _px);
    if (_dead) {
      if (r != null) NativePool.render.animClose(r.$1);
      return;
    }
    if (r == null) {
      setState(() => _handle = -1);
      return;
    }
    _handle = r.$1;
    _frames = r.$2.clamp(1, 100000);
    _fps = r.$3 > 0 ? r.$3 : 30;
    // Hamma kadr 3 MB dan oshmasa — xotirada saqlanadi.
    if (_px * _px * 4 * _frames <= 3 * 1024 * 1024) {
      _cache = List<ui.Image?>.filled(_frames, null);
    }
    if (_image == null) await _render(0);
    if (_dead) return;
    if (_frames > 1) _wantPlay();
  }

  void _wantPlay() {
    if (!widget.panel) return _play();
    if (_panelBusy < _panelSlots) {
      _panelBusy++;
      _slot = true;
      _play();
    } else {
      _panelWaiters.add(_onSlot);
    }
  }

  void _onSlot() {
    if (_dead || _slot) return;
    _panelBusy++;
    _slot = true;
    _play();
  }

  void _play() {
    _ticker ??= createTicker(_tick)..start();
  }

  void _tick(Duration t) {
    if (_busy || _handle <= 0) return;
    final f = ((t.inMicroseconds / 1e6) * _fps).floor() % _frames;
    if (f == _shown) return;
    final cached = _cache?[f];
    if (cached != null) {
      _show(cached.clone(), f);
      return;
    }
    _render(f);
  }

  Future<void> _render(int f) async {
    _busy = true;
    final bytes = await NativePool.render.animFrame(_handle, f, _px, _px);
    if (_dead || bytes == null) {
      // Ko'rinmaydigan (yashirin) kadr — oldingisi qoladi, qayta
      // so'ralmaydi (VP9 dekoderi boshidan boshlanib ketmasin).
      _shown = f;
      _busy = false;
      return;
    }
    final c = Completer<ui.Image>();
    ui.decodeImageFromPixels(
        bytes, _px, _px, ui.PixelFormat.rgba8888, c.complete);
    final img = await c.future;
    _busy = false;
    if (_dead) {
      img.dispose();
      return;
    }
    if (f == 0 && !_firstFrames.containsKey(_key)) {
      if (_firstFrames.length >= _firstFramesMax) {
        final k = _firstFrames.keys.first;
        _firstFrames.remove(k)?.dispose();
      }
      _firstFrames[_key] = img.clone();
    }
    final cache = _cache;
    if (cache != null && cache[f] == null) cache[f] = img.clone();
    _show(img, f);
  }

  void _show(ui.Image img, int f) {
    _shown = f;
    final old = _image;
    setState(() => _image = img);
    old?.dispose();
  }

  @override
  void dispose() {
    _dead = true;
    _ticker?.dispose();
    if (_handle > 0) NativePool.render.animClose(_handle);
    _image?.dispose();
    for (final i in _cache ?? const <ui.Image?>[]) {
      i?.dispose();
    }
    _panelWaiters.remove(_onSlot);
    if (_slot) {
      _panelBusy--;
      if (_panelWaiters.isNotEmpty) _panelWaiters.removeAt(0)();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final img = _image;
    if (img == null) {
      return _handle < 0 ? widget.fallback : const SizedBox.shrink();
    }
    return RawImage(
      image: img,
      width: widget.size,
      height: widget.size,
      fit: BoxFit.contain,
      filterQuality: FilterQuality.medium,
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
