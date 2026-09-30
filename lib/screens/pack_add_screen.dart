// lib/screens/pack_add_screen.dart — TO'PLAMGA QO'SHISH OYNASI.
//
// Tanlangan rasm va videolar shu yerda KO'RINIB turadi: har biri uchun
// mos emoji (ilovaning O'Z emoji oynasidan), videoni kesish, olib tashlash.
// Yuklashdan oldin hajm (<= 5 MB) va uzunlik tekshiriladi, xato sababi
// kartaning o'zida ko'rinadi. Yuklangan rasmlarni admin ko'rib chiqadi.

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';
import 'package:video_player/video_player.dart';

import '../services/pack_service.dart';
import '../services/storage_janitor.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../widgets/pack_emoji_picker.dart';
import 'pack_video_trim_screen.dart';

enum _Stage { ready, uploading, done, failed }

class _Draft {
  final String path;
  String sniffed = '';
  int size = 0;
  int durationMs = 0;
  int trimA = 0;
  int trimB = 0;
  String emoji = '';
  String? problem;
  _Stage stage = _Stage.ready;
  String status = '';

  _Draft(this.path);

  bool get isVideo => isPackVideo(sniffed);
}

class PackAddScreen extends StatefulWidget {
  final PackInfo pack;
  final List<XFile> files;
  const PackAddScreen({super.key, required this.pack, required this.files});

  @override
  State<PackAddScreen> createState() => _PackAddScreenState();
}

class _PackAddScreenState extends State<PackAddScreen> {
  final List<_Draft> _drafts = [];
  bool _busy = false;
  bool _ready = false;

  @override
  void initState() {
    super.initState();
    unawaited(_load(widget.files.map((f) => f.path).toList()));
  }

  @override
  void dispose() {
    // Tanlash paytida yasalgan vaqtinchalik nusxalar tozalanadi.
    for (final d in _drafts) {
      unawaited(StorageJanitor.dropPicked(d.path));
    }
    super.dispose();
  }

  /// Fayllarni o'qiydi: tur (baytlardan), hajm, video uzunligi.
  Future<void> _load(List<String> paths) async {
    for (final p in paths) {
      if (_drafts.any((d) => d.path == p)) continue;
      final d = _Draft(p);
      try {
        final f = File(p);
        d.size = await f.length();
        final head =
            await f.openRead(0, 32).fold<List<int>>([], (a, b) => a..addAll(b));
        d.sniffed = sniffImage(head);
        if (d.sniffed.isEmpty) {
          d.problem = 'Fayl turi qo\'llanmaydi (PNG, JPG, GIF, WebP, MP4, WebM)';
        } else if (d.size > kPackItemMaxBytes) {
          d.problem =
              '${_mb(d.size)} — 5 MB dan katta, yuklab bo\'lmaydi';
        } else if (d.isVideo) {
          final c = VideoPlayerController.file(f);
          try {
            await c.initialize();
            d.durationMs = c.value.duration.inMilliseconds;
          } catch (_) {
            d.problem = 'Videoni ochib bo\'lmadi';
          } finally {
            await c.dispose();
          }
          final max = packMaxSeconds(widget.pack.kind) * 1000;
          d.trimA = 0;
          d.trimB = d.durationMs < max ? d.durationMs : max;
        }
      } catch (_) {
        d.problem = 'Faylni o\'qib bo\'lmadi';
      }
      _drafts.add(d);
      if (mounted) setState(() {});
    }
    if (mounted) setState(() => _ready = true);
  }

  static String _mb(int b) => '${(b / 1048576).toStringAsFixed(1)} MB';

  Future<void> _more() async {
    List<XFile> more;
    try {
      more = await ImagePicker().pickMultipleMedia(limit: 20);
    } catch (_) {
      return;
    }
    if (more.isEmpty || !mounted) return;
    await _load(more.map((f) => f.path).toList());
  }

  Future<void> _pickEmoji(_Draft d) async {
    final e = await showPackEmojiPicker(context);
    if (e == null || !mounted) return;
    setState(() => d.emoji = e);
  }

  Future<void> _trim(_Draft d) async {
    final r = await Navigator.of(context).push<(int, int)>(
      MaterialPageRoute(
        builder: (_) => PackVideoTrimScreen(
          path: d.path,
          kind: widget.pack.kind,
          start: d.trimA,
          end: d.trimB,
        ),
      ),
    );
    if (r == null || !mounted) return;
    setState(() {
      d.trimA = r.$1;
      d.trimB = r.$2;
    });
  }

  int get _sendable => _drafts
      .where((d) =>
          d.problem == null && (d.stage == _Stage.ready || d.stage == _Stage.failed))
      .length;

  Future<void> _upload() async {
    if (_busy) return;
    setState(() => _busy = true);
    var ok = 0;
    for (final d in _drafts) {
      if (d.problem != null ||
          !(d.stage == _Stage.ready || d.stage == _Stage.failed)) {
        continue;
      }
      setState(() {
        d.stage = _Stage.uploading;
        d.status = '0%';
      });
      final err = await PackService.instance.addItem(
        widget.pack,
        d.path,
        emoji: d.emoji,
        trimStartMs: d.isVideo ? d.trimA : 0,
        trimEndMs: d.isVideo ? d.trimB : 0,
        onProgress: (sent, total) {
          if (!mounted || total <= 0) return;
          setState(() => d.status = '${(sent * 100 / total).floor()}%');
        },
      );
      if (!mounted) return;
      setState(() {
        if (err == null) {
          d.stage = _Stage.done;
          d.status = 'Yuborildi — admin ko\'rib chiqadi';
          ok++;
        } else {
          d.stage = _Stage.failed;
          d.status = err;
        }
      });
    }
    if (!mounted) return;
    setState(() => _busy = false);
    if (ok > 0 && _sendable == 0) {
      // Hammasi yuborildi — ozgina ko'rsatib, orqaga qaytamiz.
      await Future<void>.delayed(const Duration(milliseconds: 700));
      if (mounted) Navigator.of(context).pop(ok);
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = widget.pack;
    final n = _sendable;
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          title: Text('${PackKind.single(p.kind)} qo\'shish',
              style: const TextStyle(color: Colors.white, fontSize: 18)),
          actions: [
            IconButton(
              tooltip: 'Yana tanlash',
              onPressed: _busy ? null : _more,
              icon: const Icon(Icons.add_photo_alternate_rounded),
            ),
          ],
        ),
        bottomNavigationBar: SafeArea(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
            child: FilledButton(
              onPressed: (_busy || n == 0) ? null : _upload,
              style: FilledButton.styleFrom(
                backgroundColor: AppColors.accent,
                padding: const EdgeInsets.symmetric(vertical: 15),
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(16)),
              ),
              child: _busy
                  ? const SizedBox(
                      width: 20,
                      height: 20,
                      child: CircularProgressIndicator(
                          strokeWidth: 2, color: Colors.white))
                  : Text(n == 0 ? 'Yuboradigan narsa yo\'q' : 'Yuborish ($n)',
                      style: const TextStyle(
                          fontSize: 16, fontWeight: FontWeight.w700)),
            ),
          ),
        ),
        body: !_ready && _drafts.isEmpty
            ? const Center(
                child: CircularProgressIndicator(
                    strokeWidth: 2.4, color: Colors.white54))
            : ListView.builder(
                padding: const EdgeInsets.fromLTRB(16, 4, 16, 16),
                itemCount: _drafts.length + 1,
                itemBuilder: (_, i) {
                  if (i == 0) return _limits(p);
                  return _card(_drafts[i - 1]);
                },
              ),
      ),
    );
  }

  Widget _limits(PackInfo p) {
    final sec = packMaxSeconds(p.kind);
    return Padding(
      padding: const EdgeInsets.fromLTRB(4, 0, 4, 12),
      child: Text(
        'Rasm yoki video (har biri 5 MB gacha). Video ko\'pi bilan $sec soniya: '
        'kerakli bo\'lagini "Kesish" bilan tanlang. Admin tasdiqlagach '
        '"${p.title}" to\'plamida ko\'rinadi.',
        style: TextStyle(
            color: Colors.white.withValues(alpha: 0.55),
            fontSize: 12.5,
            height: 1.45),
      ),
    );
  }

  Widget _card(_Draft d) {
    final bad = d.problem != null;
    final locked = d.stage == _Stage.uploading || d.stage == _Stage.done;
    return Padding(
      key: ValueKey(d.path),
      padding: const EdgeInsets.only(bottom: 12),
      child: Glass(
        borderRadius: 18,
        padding: const EdgeInsets.all(12),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            ClipRRect(
              borderRadius: BorderRadius.circular(14),
              child: Container(
                width: 96,
                height: 96,
                color: Colors.white.withValues(alpha: 0.06),
                child: bad
                    ? const Icon(Icons.error_outline_rounded,
                        color: AppColors.danger)
                    : d.isVideo
                        ? PackVideoPreview(path: d.path)
                        : Image.file(
                            File(d.path),
                            fit: BoxFit.contain,
                            cacheWidth: 300,
                            gaplessPlayback: true,
                            errorBuilder: (_, __, ___) => const Icon(
                                Icons.broken_image_outlined,
                                color: Colors.white38),
                          ),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    d.isVideo
                        ? 'Video · ${_mb(d.size)}'
                        : '${d.sniffed.isEmpty ? 'Fayl' : d.sniffed.toUpperCase()} · ${_mb(d.size)}',
                    style: const TextStyle(
                        color: Colors.white,
                        fontSize: 14,
                        fontWeight: FontWeight.w600),
                  ),
                  if (bad) ...[
                    const SizedBox(height: 4),
                    Text(d.problem!,
                        style: const TextStyle(
                            color: AppColors.danger, fontSize: 12.5)),
                  ] else ...[
                    if (d.isVideo)
                      Padding(
                        padding: const EdgeInsets.only(top: 3),
                        child: Text(
                          'Bo\'lak: ${fmtMs(d.trimA)} – ${fmtMs(d.trimB)} '
                          '(${((d.trimB - d.trimA) / 1000).toStringAsFixed(1)} s)',
                          style: const TextStyle(
                              color: Colors.white60, fontSize: 12.5),
                        ),
                      ),
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 8,
                      runSpacing: 6,
                      children: [
                        _chip(
                          onTap: locked ? null : () => _pickEmoji(d),
                          child: d.emoji.isEmpty
                              ? const Text('Emoji',
                                  style: TextStyle(
                                      color: Colors.white70, fontSize: 13))
                              : PackEmojiLabel(d.emoji, size: 20),
                          icon: d.emoji.isEmpty
                              ? Icons.emoji_emotions_outlined
                              : null,
                        ),
                        if (d.isVideo)
                          _chip(
                            onTap: locked ? null : () => _trim(d),
                            icon: Icons.content_cut_rounded,
                            child: const Text('Kesish',
                                style: TextStyle(
                                    color: Colors.white70, fontSize: 13)),
                          ),
                      ],
                    ),
                  ],
                  if (d.stage != _Stage.ready) ...[
                    const SizedBox(height: 8),
                    Text(
                      d.status,
                      style: TextStyle(
                          color: d.stage == _Stage.failed
                              ? AppColors.danger
                              : d.stage == _Stage.done
                                  ? AppColors.success
                                  : AppColors.gold,
                          fontSize: 12.5),
                    ),
                  ],
                ],
              ),
            ),
            if (!locked)
              IconButton(
                visualDensity: VisualDensity.compact,
                onPressed: _busy
                    ? null
                    : () {
                        setState(() => _drafts.remove(d));
                        unawaited(StorageJanitor.dropPicked(d.path));
                      },
                icon: const Icon(Icons.close_rounded, color: Colors.white54),
              ),
          ],
        ),
      ),
    );
  }

  Widget _chip({VoidCallback? onTap, IconData? icon, required Widget child}) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 7),
        decoration: BoxDecoration(
          color: Colors.white.withValues(alpha: onTap == null ? 0.04 : 0.09),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[
              Icon(icon, size: 16, color: Colors.white70),
              const SizedBox(width: 6),
            ],
            child,
          ],
        ),
      ),
    );
  }
}
