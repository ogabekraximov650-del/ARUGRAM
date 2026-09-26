// lib/widgets/tg_attach_sheet.dart — TELEGRAM'DAGIDEK BIRIKTIRISH OYNASI.
//
// TALAB (foydalanuvchi): "fayl yuborish tugmasini bosganda huddi
// Telegram'dagidek ilovaning o'zidan galereya va fayl yuboradigan
// oynalar ochilsin".
//
// Telegram/Cherrygram `ChatAttachAlert` kabi:
//   * pastdan chiqadigan, tortib kattalashtiriladigan oyna;
//   * ilova ICHIDAGI galereya to'ri (3 ustun), birinchi katakda
//     jonli kamera; videoda uzunligi; o'ng yuqorida tanlash doirasi
//     (tanlash tartibi raqam bilan);
//   * tepada albom tanlash ("Galereya ▾");
//   * pastda "Galereya | Fayl" tugmalari; biror narsa tanlanganda
//     ularning o'rnida izoh maydoni va ➤ (nechta tanlangani bilan).

import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:camera/camera.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';
import 'package:photo_manager/photo_manager.dart';

import 'glass.dart';

/// Tanlangan bitta narsa.
class TgAttachItem {
  final File file;

  /// `image`, `video` yoki `file`.
  final String type;
  final String name;
  final int durationMs;
  const TgAttachItem(this.file, this.type, this.name, {this.durationMs = 0});
}

class TgAttachResult {
  final List<TgAttachItem> items;
  final String caption;
  const TgAttachResult(this.items, this.caption);
}

Future<TgAttachResult?> showTgAttachSheet(BuildContext context) {
  return showModalBottomSheet<TgAttachResult>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    backgroundColor: Colors.transparent,
    barrierColor: Colors.black54,
    builder: (_) => const _AttachSheet(),
  );
}

abstract final class _C {
  static const bg = Color(0xFF1C1F24);
  static const bar = Color(0xFF23272D);
  static const hint = Color(0xFF8A939D);
  static const blue = Color(0xFF3D9AEA);
}

class _AttachSheet extends StatefulWidget {
  const _AttachSheet();

  @override
  State<_AttachSheet> createState() => _AttachSheetState();
}

class _AttachSheetState extends State<_AttachSheet> {
  final _caption = TextEditingController();
  int _tab = 0;
  PermissionState? _perm;
  List<AssetPathEntity> _albums = [];
  AssetPathEntity? _album;
  final List<AssetEntity> _items = [];
  int _page = 0;
  bool _loading = false;
  bool _end = false;
  final List<AssetEntity> _selected = [];
  CameraController? _cam;
  bool _sending = false;

  @override
  void initState() {
    super.initState();
    _init();
    _startCamera();
  }

  @override
  void dispose() {
    _caption.dispose();
    _cam?.dispose();
    super.dispose();
  }

  Future<void> _init() async {
    final p = await PhotoManager.requestPermissionExtend();
    if (!mounted) return;
    setState(() => _perm = p);
    if (!p.hasAccess) return;
    final albums = await PhotoManager.getAssetPathList(
      type: RequestType.common,
      hasAll: true,
    );
    if (!mounted) return;
    setState(() {
      _albums = albums;
      _album = albums.isEmpty ? null : albums.first;
    });
    _more();
  }

  /// Birinchi katakdagi jonli kamera (Telegram'dagidek).
  Future<void> _startCamera() async {
    try {
      final cams = await availableCameras();
      if (cams.isEmpty) return;
      final back = cams.firstWhere(
          (c) => c.lensDirection == CameraLensDirection.back,
          orElse: () => cams.first);
      final c = CameraController(back, ResolutionPreset.low,
          enableAudio: false);
      await c.initialize();
      if (!mounted) {
        await c.dispose();
        return;
      }
      setState(() => _cam = c);
    } catch (_) {
      // Ruxsat yo'q — katakda oddiy kamera belgisi qoladi.
    }
  }

  Future<void> _more() async {
    final a = _album;
    if (a == null || _loading || _end) return;
    _loading = true;
    final list = await a.getAssetListPaged(page: _page, size: 60);
    _loading = false;
    if (!mounted) return;
    setState(() {
      _items.addAll(list);
      _page++;
      _end = list.length < 60;
    });
  }

  void _pickAlbum(AssetPathEntity a) {
    setState(() {
      _album = a;
      _items.clear();
      _page = 0;
      _end = false;
    });
    _more();
  }

  void _toggle(AssetEntity e) {
    setState(() {
      if (!_selected.remove(e)) {
        if (_selected.length < 10) _selected.add(e);
      }
    });
  }

  Future<void> _shoot() async {
    await _cam?.dispose();
    if (mounted) setState(() => _cam = null);
    final x = await ImagePicker()
        .pickImage(source: ImageSource.camera, imageQuality: 85);
    if (!mounted) return;
    if (x == null) {
      _startCamera();
      return;
    }
    Navigator.of(context).pop(TgAttachResult(
        [TgAttachItem(File(x.path), 'image', x.name)], _caption.text.trim()));
  }

  Future<void> _send() async {
    if (_sending || _selected.isEmpty) return;
    setState(() => _sending = true);
    final out = <TgAttachItem>[];
    for (final e in _selected) {
      final f = await e.originFile ?? await e.file;
      if (f == null) continue;
      final video = e.type == AssetType.video;
      out.add(TgAttachItem(f, video ? 'video' : 'image',
          e.title ?? f.path.split('/').last,
          durationMs: video ? e.duration * 1000 : 0));
    }
    if (!mounted) return;
    Navigator.of(context).pop(TgAttachResult(out, _caption.text.trim()));
  }

  Future<void> _pickFiles(FileType type) async {
    final r = await FilePicker.platform.pickFiles(
      type: type,
      allowMultiple: true,
    );
    if (!mounted || r == null) return;
    final items = <TgAttachItem>[];
    for (final f in r.files) {
      final p = f.path;
      if (p == null) continue;
      items.add(TgAttachItem(File(p), 'file', f.name));
    }
    if (items.isEmpty) return;
    Navigator.of(context).pop(TgAttachResult(items, ''));
  }

  @override
  Widget build(BuildContext context) {
    final bottomInset = MediaQuery.viewInsetsOf(context).bottom;
    return Padding(
      padding: EdgeInsets.only(bottom: bottomInset),
      child: DraggableScrollableSheet(
        initialChildSize: 0.6,
        minChildSize: 0.35,
        maxChildSize: 1,
        expand: false,
        snap: true,
        builder: (context, scroll) => ClipRRect(
          borderRadius: const BorderRadius.vertical(top: Radius.circular(14)),
          child: Container(
            color: _C.bg,
            child: Column(
              children: [
                _header(),
                Expanded(
                  child: _tab == 0 ? _gallery(scroll) : _files(scroll),
                ),
                _selected.isNotEmpty && _tab == 0 ? _captionBar() : _tabs(),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _header() {
    return SizedBox(
      height: 52,
      child: Stack(
        children: [
          Align(
            alignment: Alignment.topCenter,
            child: Container(
              margin: const EdgeInsets.only(top: 6),
              width: 36,
              height: 4,
              decoration: BoxDecoration(
                color: Colors.white24,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
          ),
          Positioned.fill(
            top: 10,
            child: Row(
              children: [
                IconButton(
                  icon: const Icon(Icons.close, color: Colors.white),
                  onPressed: () => Navigator.of(context).pop(),
                ),
                if (_tab == 0 && _albums.isNotEmpty)
                  PopupMenuButton<AssetPathEntity>(
                    color: _C.bar,
                    onSelected: _pickAlbum,
                    itemBuilder: (_) => [
                      for (final a in _albums)
                        PopupMenuItem(
                          value: a,
                          child: Text(a.isAll ? 'Galereya' : a.name,
                              style: const TextStyle(color: Colors.white)),
                        ),
                    ],
                    child: Row(
                      children: [
                        Text(
                          _selected.isNotEmpty
                              ? '${_selected.length} ta tanlandi'
                              : (_album == null || _album!.isAll
                                  ? 'Galereya'
                                  : _album!.name),
                          style: const TextStyle(
                              color: Colors.white,
                              fontSize: 18,
                              fontWeight: FontWeight.w600),
                        ),
                        const Icon(Icons.arrow_drop_down, color: Colors.white),
                      ],
                    ),
                  )
                else
                  Text(_tab == 0 ? 'Galereya' : 'Fayl',
                      style: const TextStyle(
                          color: Colors.white,
                          fontSize: 18,
                          fontWeight: FontWeight.w600)),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _gallery(ScrollController scroll) {
    final p = _perm;
    if (p != null && !p.hasAccess) {
      return ListView(
        controller: scroll,
        padding: const EdgeInsets.all(28),
        children: [
          const Icon(Icons.photo_library_outlined, color: _C.hint, size: 56),
          const SizedBox(height: 14),
          const Text(
            'Rasm va videolarni yuborish uchun galereyaga ruxsat bering',
            textAlign: TextAlign.center,
            style: TextStyle(color: Colors.white, fontSize: 15, height: 1.4),
          ),
          const SizedBox(height: 14),
          Center(
            child: FilledButton(
              style: FilledButton.styleFrom(backgroundColor: _C.blue),
              onPressed: () async {
                await PhotoManager.openSetting();
              },
              child: const Text('Sozlamalarni ochish'),
            ),
          ),
        ],
      );
    }
    return NotificationListener<ScrollNotification>(
      onNotification: (n) {
        if (n.metrics.pixels > n.metrics.maxScrollExtent - 600) _more();
        return false;
      },
      child: GridView.builder(
        controller: scroll,
        padding: const EdgeInsets.all(2),
        gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
          crossAxisCount: 3,
          mainAxisSpacing: 2,
          crossAxisSpacing: 2,
        ),
        itemCount: _items.length + 1,
        itemBuilder: (_, i) {
          if (i == 0) return _cameraTile();
          final e = _items[i - 1];
          final n = _selected.indexOf(e);
          return _AssetTile(
            asset: e,
            number: n < 0 ? null : n + 1,
            onTap: () => _toggle(e),
          );
        },
      ),
    );
  }

  Widget _cameraTile() {
    final c = _cam;
    return GestureDetector(
      onTap: _shoot,
      child: Container(
        color: Colors.black,
        child: Stack(
          fit: StackFit.expand,
          children: [
            if (c != null && c.value.isInitialized)
              ClipRect(
                child: FittedBox(
                  fit: BoxFit.cover,
                  child: SizedBox(
                    width: c.value.previewSize?.height ?? 100,
                    height: c.value.previewSize?.width ?? 100,
                    child: CameraPreview(c),
                  ),
                ),
              ),
            const Center(
              child: Icon(Icons.photo_camera_outlined,
                  color: Colors.white, size: 30),
            ),
          ],
        ),
      ),
    );
  }

  Widget _files(ScrollController scroll) {
    Widget row(IconData icon, Color color, String title, String sub,
        VoidCallback onTap) {
      return ListTile(
        onTap: onTap,
        leading: CircleAvatar(
          backgroundColor: color,
          child: Icon(icon, color: Colors.white),
        ),
        title: Text(title, style: const TextStyle(color: Colors.white)),
        subtitle: Text(sub, style: const TextStyle(color: _C.hint)),
      );
    }

    return ListView(
      controller: scroll,
      children: [
        row(Icons.folder_rounded, _C.blue, 'Ichki xotira',
            'Istalgan faylni tanlash', () => _pickFiles(FileType.any)),
        row(Icons.image_rounded, const Color(0xFF4CAF50), 'Galereya',
            'Rasm va videoni SIQILMAGAN fayl sifatida',
            () => _pickFiles(FileType.media)),
        row(Icons.music_note_rounded, const Color(0xFFFF7043), 'Musiqa',
            'Audio fayllar', () => _pickFiles(FileType.audio)),
      ],
    );
  }

  Widget _tabs() {
    Widget tab(int i, IconData icon, Color color, String label) {
      final on = _tab == i;
      return Expanded(
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: () => setState(() => _tab = i),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              AnimatedContainer(
                duration: const Duration(milliseconds: 160),
                width: 48,
                height: 48,
                decoration: BoxDecoration(
                  color: on ? color : color.withValues(alpha: 0.18),
                  shape: BoxShape.circle,
                ),
                child: Icon(icon, color: on ? Colors.white : color, size: 24),
              ),
              const SizedBox(height: 4),
              Text(label,
                  style: TextStyle(
                      color: on ? Colors.white : _C.hint, fontSize: 12.5)),
            ],
          ),
        ),
      );
    }

    return Container(
      color: _C.bar,
      padding: EdgeInsets.fromLTRB(
          40, 10, 40, 10 + MediaQuery.paddingOf(context).bottom),
      child: Row(
        children: [
          tab(0, Icons.image_rounded, _C.blue, 'Galereya'),
          tab(1, Icons.insert_drive_file_rounded, const Color(0xFF4CAF50),
              'Fayl'),
        ],
      ),
    );
  }

  Widget _captionBar() {
    return Container(
      color: _C.bar,
      padding: EdgeInsets.fromLTRB(
          12, 8, 12, 8 + MediaQuery.paddingOf(context).bottom),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Expanded(
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 14),
              decoration: BoxDecoration(
                color: Colors.white.withValues(alpha: 0.07),
                borderRadius: BorderRadius.circular(22),
              ),
              child: TextField(
                controller: _caption,
                minLines: 1,
                maxLines: 4,
                style: const TextStyle(color: Colors.white, fontSize: 15),
                decoration: const InputDecoration(
                  border: InputBorder.none,
                  hintText: 'Izoh qo\'shish...',
                  hintStyle: TextStyle(color: _C.hint),
                ),
              ),
            ),
          ),
          const SizedBox(width: 10),
          GestureDetector(
            onTap: _send,
            child: Stack(
              clipBehavior: Clip.none,
              children: [
                Container(
                  width: 48,
                  height: 48,
                  decoration: const BoxDecoration(
                      color: AppColors.accent, shape: BoxShape.circle),
                  child: _sending
                      ? const Padding(
                          padding: EdgeInsets.all(14),
                          child: CircularProgressIndicator(
                              strokeWidth: 2, color: Colors.white),
                        )
                      : const Icon(Icons.send_rounded, color: Colors.white),
                ),
                Positioned(
                  right: -2,
                  top: -4,
                  child: Container(
                    padding:
                        const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
                    decoration: BoxDecoration(
                      color: Colors.white,
                      borderRadius: BorderRadius.circular(10),
                      border: Border.all(color: _C.bar, width: 2),
                    ),
                    child: Text('${_selected.length}',
                        style: const TextStyle(
                            color: AppColors.accent,
                            fontSize: 12,
                            fontWeight: FontWeight.w700)),
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

/// Galereyadagi bitta katak: kichik rasm, videoda uzunlik, tanlash
/// doirasi (tanlash tartibi raqami bilan).
class _AssetTile extends StatefulWidget {
  final AssetEntity asset;
  final int? number;
  final VoidCallback onTap;
  const _AssetTile(
      {required this.asset, required this.number, required this.onTap});

  @override
  State<_AssetTile> createState() => _AssetTileState();
}

class _AssetTileState extends State<_AssetTile> {
  static final Map<String, Future<Uint8List?>> _thumbs = {};
  late Future<Uint8List?> _thumb;

  @override
  void initState() {
    super.initState();
    _thumb = _thumbs[widget.asset.id] ??= widget.asset
        .thumbnailDataWithSize(const ThumbnailSize.square(240), quality: 80);
  }

  String _dur(int s) =>
      '${s ~/ 60}:${(s % 60).toString().padLeft(2, '0')}';

  @override
  Widget build(BuildContext context) {
    final e = widget.asset;
    final n = widget.number;
    return GestureDetector(
      onTap: widget.onTap,
      child: Stack(
        fit: StackFit.expand,
        children: [
          FutureBuilder<Uint8List?>(
            future: _thumb,
            builder: (_, s) => s.data == null
                ? Container(color: Colors.white.withValues(alpha: 0.05))
                : AnimatedScale(
                    scale: n != null ? 0.86 : 1,
                    duration: const Duration(milliseconds: 150),
                    child: Image.memory(s.data!,
                        fit: BoxFit.cover, gaplessPlayback: true),
                  ),
          ),
          if (e.type == AssetType.video)
            Positioned(
              left: 5,
              bottom: 5,
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                decoration: BoxDecoration(
                  color: Colors.black54,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Icon(Icons.videocam_rounded,
                        size: 12, color: Colors.white),
                    const SizedBox(width: 2),
                    Text(_dur(e.duration),
                        style: const TextStyle(
                            color: Colors.white, fontSize: 11)),
                  ],
                ),
              ),
            ),
          Positioned(
            right: 5,
            top: 5,
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 150),
              width: 24,
              height: 24,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: n != null ? _C.blue : Colors.black26,
                border: Border.all(color: Colors.white, width: 1.6),
              ),
              child: n == null
                  ? null
                  : Text('$n',
                      style: const TextStyle(
                          color: Colors.white,
                          fontSize: 12,
                          fontWeight: FontWeight.w700)),
            ),
          ),
        ],
      ),
    );
  }
}
