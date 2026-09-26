// lib/widgets/tg_attach_sheet.dart — TELEGRAM'DAGIDEK BIRIKTIRISH OYNASI.
//
// TALAB (foydalanuvchi): "Fayl yuborish va galereya oynasini
// Telegram'nikidek qilib qayta qur".
//
// Manba: Cherrygram `ChatAttachAlert.java`, `ChatAttachAlertPhotoLayout`,
// `PhotoAttachPhotoCell`, `CheckBoxBase`, `glass/GlassTabView`:
//
//   * pastdan chiqadigan, tortib kattalashtiriladigan oyna;
//   * galereya to'ri: 3 ustun, chet va oraliq 2 dp; birinchi katakda
//     jonli kamera; videoda chap pastda (4 dp) 17 dp lik qoramtir
//     yorliq — ▶ va uzunligi (12, qalin);
//   * o'ng yuqorida (5 dp) 24 dp lik tanlash doirasi: bo'sh — oq
//     halqa, tanlangan — urg'u rangida, oq hoshiyali, ichida TARTIB
//     RAQAMI; tanlangan rasm 0.787 gacha kichrayadi;
//   * tepada: ✕ va albom tanlash ("Galereya ▾"), tanlanganda
//     "N ta tanlandi";
//   * pastda SHISHA "tabletka" (balandligi 56, radiusi 28) — Galereya,
//     Fayl, Musiqa: har biri Lottie belgi (24) + nom (11, qalin),
//     tanlangani yumshoq fon bilan (`tab_*.json` / `tab_*_reverse.json`);
//     orqasida 72 dp lik xiralashuvchi pastki qatlam;
//   * biror narsa tanlanganda tablar o'rnida izoh maydoni va ➤ (nechta
//     tanlangani nishonchada).

import 'dart:async';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:camera/camera.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';
import 'package:lottie/lottie.dart';
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
  static const bg = AppColors.card;
  static const hint = Color(0xFF8A939D);

  /// `glass_tabUnselected`.
  static const tab = Color(0xFFAAB0B7);
  static const accent = AppColors.accent2;
}

/// Bo'limlar (Telegram'dagi tartibda).
enum _Tab { gallery, file, music }

class _AttachSheet extends StatefulWidget {
  const _AttachSheet();

  @override
  State<_AttachSheet> createState() => _AttachSheetState();
}

class _AttachSheetState extends State<_AttachSheet> {
  static const _max = 10;

  final _caption = TextEditingController();
  _Tab _tab = _Tab.gallery;
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

  /// Musiqa bo'limi (qurilmadagi audio fayllar).
  List<AssetEntity>? _songs;
  final List<AssetEntity> _pickedSongs = [];

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

  /// Birinchi katakdagi jonli kamera (`PhotoAttachCameraCell`).
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

  Future<void> _loadSongs() async {
    if (_songs != null) return;
    try {
      final p = _perm ?? await PhotoManager.requestPermissionExtend();
      if (!p.hasAccess) {
        if (mounted) setState(() => _songs = const []);
        return;
      }
      final paths = await PhotoManager.getAssetPathList(
          type: RequestType.audio, hasAll: true, onlyAll: true);
      final list = paths.isEmpty
          ? <AssetEntity>[]
          : await paths.first.getAssetListRange(start: 0, end: 500);
      if (mounted) setState(() => _songs = list);
    } catch (_) {
      if (mounted) setState(() => _songs = const []);
    }
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
        if (_selected.length < _max) _selected.add(e);
      }
    });
  }

  void _toggleSong(AssetEntity e) {
    setState(() {
      if (!_pickedSongs.remove(e)) {
        if (_pickedSongs.length < _max) _pickedSongs.add(e);
      }
    });
  }

  void _setTab(_Tab t) {
    if (t == _tab) return;
    setState(() => _tab = t);
    if (t == _Tab.music) _loadSongs();
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
    final music = _tab == _Tab.music;
    final list = music ? _pickedSongs : _selected;
    if (_sending || list.isEmpty) return;
    setState(() => _sending = true);
    final out = <TgAttachItem>[];
    for (final e in list) {
      final f = await e.originFile ?? await e.file;
      if (f == null) continue;
      final video = e.type == AssetType.video;
      out.add(TgAttachItem(
          f,
          music ? 'file' : (video ? 'video' : 'image'),
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

  int get _count =>
      _tab == _Tab.music ? _pickedSongs.length : _selected.length;

  @override
  Widget build(BuildContext context) {
    final bottomInset = MediaQuery.viewInsetsOf(context).bottom;
    final safe = MediaQuery.paddingOf(context).bottom;
    final showCaption = _count > 0 && _tab != _Tab.file;
    return Padding(
      padding: EdgeInsets.only(bottom: bottomInset),
      child: DraggableScrollableSheet(
        initialChildSize: 0.62,
        minChildSize: 0.35,
        maxChildSize: 1,
        expand: false,
        snap: true,
        builder: (context, scroll) => ClipRRect(
          borderRadius: const BorderRadius.vertical(top: Radius.circular(14)),
          child: ColoredBox(
            color: _C.bg,
            child: Stack(
              children: [
                Column(
                  children: [
                    _header(),
                    Expanded(
                      child: AnimatedSwitcher(
                        duration: const Duration(milliseconds: 200),
                        child: KeyedSubtree(
                          key: ValueKey(_tab),
                          child: switch (_tab) {
                            _Tab.gallery => _gallery(scroll, safe),
                            _Tab.file => _files(scroll, safe),
                            _Tab.music => _music(scroll, safe),
                          },
                        ),
                      ),
                    ),
                  ],
                ),
                // Pastki xiralashuvchi qatlam (`bottomFadeDrawable`,
                // 72 dp) — tablar ostida kontent sekin so'nadi.
                Positioned(
                  left: 0,
                  right: 0,
                  bottom: 0,
                  height: 72 + safe + 20,
                  child: IgnorePointer(
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        gradient: LinearGradient(
                          begin: Alignment.topCenter,
                          end: Alignment.bottomCenter,
                          colors: [
                            _C.bg.withValues(alpha: 0),
                            _C.bg.withValues(alpha: 0.92),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
                Positioned(
                  left: 0,
                  right: 0,
                  bottom: 0,
                  child: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 220),
                    transitionBuilder: (c, a) => FadeTransition(
                      opacity: a,
                      child: SlideTransition(
                        position: Tween(
                                begin: const Offset(0, 0.3), end: Offset.zero)
                            .animate(a),
                        child: c,
                      ),
                    ),
                    child: showCaption
                        ? KeyedSubtree(
                            key: const ValueKey('caption'),
                            child: _captionBar(safe))
                        : KeyedSubtree(
                            key: const ValueKey('tabs'), child: _tabs(safe)),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  // ── TEPA QISM ─────────────────────────────────────────────────
  Widget _header() {
    final title = switch (_tab) {
      _Tab.gallery => _selected.isNotEmpty
          ? '${_selected.length} ta tanlandi'
          : (_album == null || _album!.isAll ? 'Galereya' : _album!.name),
      _Tab.file => 'Fayl tanlash',
      _Tab.music => _pickedSongs.isNotEmpty
          ? '${_pickedSongs.length} ta tanlandi'
          : 'Musiqa',
    };
    final titleText = Text(title,
        style: const TextStyle(
            color: Colors.white, fontSize: 20, fontWeight: FontWeight.w600));
    return SizedBox(
      height: 56,
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
            top: 8,
            child: Row(
              children: [
                const SizedBox(width: 4),
                IconButton(
                  icon: const Icon(Icons.close_rounded, color: Colors.white),
                  onPressed: () => Navigator.of(context).pop(),
                ),
                const SizedBox(width: 4),
                if (_tab == _Tab.gallery &&
                    _albums.length > 1 &&
                    _selected.isEmpty)
                  PopupMenuButton<AssetPathEntity>(
                    color: AppColors.cardAlt,
                    onSelected: _pickAlbum,
                    shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(12)),
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
                        titleText,
                        const Icon(Icons.arrow_drop_down_rounded,
                            color: Colors.white),
                      ],
                    ),
                  )
                else
                  AnimatedSwitcher(
                    duration: const Duration(milliseconds: 180),
                    child: KeyedSubtree(key: ValueKey(title), child: titleText),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  // ── GALEREYA (`ChatAttachAlertPhotoLayout`) ───────────────────
  Widget _gallery(ScrollController scroll, double safe) {
    final p = _perm;
    if (p != null && !p.hasAccess) {
      return _NoAccess(scroll: scroll);
    }
    return NotificationListener<ScrollNotification>(
      onNotification: (n) {
        if (n.metrics.pixels > n.metrics.maxScrollExtent - 600) _more();
        return false;
      },
      child: GridView.builder(
        controller: scroll,
        padding: EdgeInsets.fromLTRB(2, 0, 2, 96 + safe),
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
              child: Icon(Icons.photo_camera_rounded,
                  color: Colors.white, size: 28),
            ),
          ],
        ),
      ),
    );
  }

  // ── FAYL (`ChatAttachAlertDocumentLayout`) ────────────────────
  Widget _files(ScrollController scroll, double safe) {
    return ListView(
      controller: scroll,
      padding: EdgeInsets.only(bottom: 96 + safe),
      children: [
        _DocRow(
          icon: Icons.folder_rounded,
          color: const Color(0xFF3D9AEA),
          title: 'Ichki xotira',
          subtitle: 'Fayl tizimidan istalgan fayl',
          onTap: () => _pickFiles(FileType.any),
        ),
        _DocRow(
          icon: Icons.image_rounded,
          color: const Color(0xFF4FC76A),
          title: 'Galereya',
          subtitle: 'Rasm va videoni siqilmagan holda yuborish',
          onTap: () => _pickFiles(FileType.media),
        ),
        _DocRow(
          icon: Icons.music_note_rounded,
          color: const Color(0xFFF07F3A),
          title: 'Musiqa',
          subtitle: 'Audio fayllar',
          onTap: () => _pickFiles(FileType.audio),
          divider: false,
        ),
        const _SectionShadow(),
        const Padding(
          padding: EdgeInsets.fromLTRB(21, 14, 21, 8),
          child: Text(
            'Yuborilgan fayllar serverda shifrlangan holda saqlanadi.',
            style: TextStyle(color: _C.hint, fontSize: 13.5, height: 1.35),
          ),
        ),
      ],
    );
  }

  // ── MUSIQA (`ChatAttachAlertAudioLayout`) ─────────────────────
  Widget _music(ScrollController scroll, double safe) {
    final songs = _songs;
    if (songs == null) {
      return const Center(
          child: CircularProgressIndicator(strokeWidth: 2, color: _C.accent));
    }
    if (songs.isEmpty) {
      return ListView(
        controller: scroll,
        padding: const EdgeInsets.all(32),
        children: [
          const Icon(Icons.music_off_rounded, color: _C.hint, size: 56),
          const SizedBox(height: 12),
          const Text('Qurilmada musiqa topilmadi',
              textAlign: TextAlign.center,
              style: TextStyle(color: Colors.white, fontSize: 15)),
          const SizedBox(height: 12),
          Center(
            child: TextButton(
              onPressed: () => _pickFiles(FileType.audio),
              child: const Text('Fayldan tanlash',
                  style: TextStyle(color: _C.accent)),
            ),
          ),
        ],
      );
    }
    return ListView.builder(
      controller: scroll,
      padding: EdgeInsets.only(bottom: 96 + safe),
      itemCount: songs.length,
      itemBuilder: (_, i) {
        final s = songs[i];
        final n = _pickedSongs.indexOf(s);
        return _SongRow(
          song: s,
          number: n < 0 ? null : n + 1,
          onTap: () => _toggleSong(s),
        );
      },
    );
  }

  // ── PASTDAGI SHISHA TABLAR (`GlassTabView`) ───────────────────
  Widget _tabs(double safe) {
    return Padding(
      padding: EdgeInsets.only(bottom: 8 + safe),
      child: Center(
        child: _GlassPill(
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              _AttachTab(
                label: 'Galereya',
                anim: 'tab_gallery',
                selected: _tab == _Tab.gallery,
                onTap: () => _setTab(_Tab.gallery),
              ),
              _AttachTab(
                label: 'Fayl',
                anim: 'tab_files',
                selected: _tab == _Tab.file,
                onTap: () => _setTab(_Tab.file),
              ),
              _AttachTab(
                label: 'Musiqa',
                anim: 'tab_music',
                selected: _tab == _Tab.music,
                onTap: () => _setTab(_Tab.music),
              ),
            ],
          ),
        ),
      ),
    );
  }

  // ── IZOH VA ➤ (`commentTextView` + `writeButton`) ─────────────
  Widget _captionBar(double safe) {
    return Padding(
      padding: EdgeInsets.fromLTRB(8, 0, 8, 8 + safe),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Expanded(
            child: _GlassPill(
              radius: 24,
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: TextField(
                controller: _caption,
                minLines: 1,
                maxLines: 4,
                style: const TextStyle(color: Colors.white, fontSize: 16),
                cursorColor: _C.accent,
                decoration: const InputDecoration(
                  border: InputBorder.none,
                  isDense: true,
                  contentPadding: EdgeInsets.symmetric(vertical: 14),
                  hintText: 'Izoh qo\'shish...',
                  hintStyle: TextStyle(color: _C.hint),
                ),
              ),
            ),
          ),
          const SizedBox(width: 8),
          _SendButton(
            count: _count,
            busy: _sending,
            onTap: _send,
          ),
        ],
      ),
    );
  }
}

/// Shisha "tabletka" (`BlurredBackgroundDrawable`, radiusi 28).
class _GlassPill extends StatelessWidget {
  final Widget child;
  final double radius;
  final EdgeInsets padding;

  const _GlassPill({
    required this.child,
    this.radius = 28,
    this.padding = const EdgeInsets.all(4),
  });

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(radius),
      child: BackdropFilter(
        filter: ui.ImageFilter.blur(sigmaX: 18, sigmaY: 18),
        child: Container(
          padding: padding,
          decoration: BoxDecoration(
            color: const Color(0xCC26292E),
            borderRadius: BorderRadius.circular(radius),
            border: Border.all(color: Colors.white.withValues(alpha: 0.08)),
          ),
          child: child,
        ),
      ),
    );
  }
}

/// Bitta tab: Lottie belgi (24) + nom (11, qalin). Tanlanganda belgi
/// "to'ladi" (`tab_*.json`), tanlov olinsa qaytadi (`*_reverse.json`),
/// orqasida urg'u rangidagi yumshoq tabletka 0.6 dan 1 gacha o'sadi.
class _AttachTab extends StatefulWidget {
  final String label;
  final String anim;
  final bool selected;
  final VoidCallback onTap;

  const _AttachTab({
    required this.label,
    required this.anim,
    required this.selected,
    required this.onTap,
  });

  @override
  State<_AttachTab> createState() => _AttachTabState();
}

class _AttachTabState extends State<_AttachTab> with TickerProviderStateMixin {
  late final AnimationController _sel = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 320),
      value: widget.selected ? 1 : 0);
  late final AnimationController _icon = AnimationController(vsync: this);
  late bool _reverse = !widget.selected;

  @override
  void initState() {
    super.initState();
    // Birinchi ko'rinishda — oxirgi kadr (animatsiyasiz).
    _icon.value = 1;
  }

  @override
  void didUpdateWidget(_AttachTab old) {
    super.didUpdateWidget(old);
    if (old.selected != widget.selected) {
      if (widget.selected) {
        _sel.forward();
      } else {
        _sel.reverse();
      }
      setState(() => _reverse = !widget.selected);
      // Yangi fayl yuklangach (`onLoaded`) o'ynaydi; oldin yuklangan
      // bo'lsa — darhol.
      _icon.value = 0;
      if (_icon.duration != null) _icon.forward();
    }
  }

  @override
  void dispose() {
    _sel.dispose();
    _icon.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final w = _tabWidth(widget.label);
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: widget.onTap,
      child: AnimatedBuilder(
        animation: _sel,
        builder: (context, _) {
          final t = Curves.decelerate.transform(_sel.value);
          final color = Color.lerp(_C.tab, _C.accent, t)!;
          return SizedBox(
            width: w,
            height: 48,
            child: Stack(
              alignment: Alignment.center,
              children: [
                if (t > 0)
                  Transform.scale(
                    scale: ui.lerpDouble(0.6, 1, t),
                    child: Container(
                      decoration: BoxDecoration(
                        color: _C.accent.withValues(alpha: 0.12 * t),
                        borderRadius: BorderRadius.circular(24),
                      ),
                    ),
                  ),
                Positioned(
                  top: 4,
                  child: ColorFiltered(
                    colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
                    child: Lottie.asset(
                      'assets/tg_anim/${widget.anim}${_reverse ? '_reverse' : ''}.json',
                      key: ValueKey(_reverse),
                      controller: _icon,
                      width: 24,
                      height: 24,
                      onLoaded: (c) {
                        _icon.duration = c.duration;
                        if (_icon.value < 1 && !_icon.isAnimating) {
                          _icon.forward();
                        }
                      },
                    ),
                  ),
                ),
                Positioned(
                  top: 29,
                  left: 0,
                  right: 0,
                  child: Text(
                    widget.label,
                    textAlign: TextAlign.center,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: color,
                      fontSize: 11,
                      fontWeight:
                          widget.selected ? FontWeight.w800 : FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ),
          );
        },
      ),
    );
  }

  /// `measureAttachTabWidth`: matn + 16..8 dp chet, eng ko'pi 84.
  static double _tabWidth(String s) {
    final tp = TextPainter(
      text: TextSpan(
          text: s,
          style: const TextStyle(fontSize: 11, fontWeight: FontWeight.w600)),
      textDirection: TextDirection.ltr,
    )..layout();
    final pad = ui.lerpDouble(16, 8, ((tp.width - 40) / 16).clamp(0.0, 1.0))!;
    return (tp.width + pad * 2).clamp(0.0, 84.0) + 8;
  }
}

/// ➤ tugmasi va nechta tanlangani (`writeButton` + `selectedCountView`).
class _SendButton extends StatelessWidget {
  final int count;
  final bool busy;
  final VoidCallback onTap;

  const _SendButton(
      {required this.count, required this.busy, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: SizedBox(
        width: 52,
        height: 52,
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            Container(
              width: 52,
              height: 52,
              decoration: const BoxDecoration(
                  color: _C.accent, shape: BoxShape.circle),
              child: busy
                  ? const Padding(
                      padding: EdgeInsets.all(15),
                      child: CircularProgressIndicator(
                          strokeWidth: 2, color: Colors.white),
                    )
                  : const Padding(
                      padding: EdgeInsets.only(left: 3),
                      child: Icon(Icons.send_rounded,
                          color: Colors.white, size: 24),
                    ),
            ),
            Positioned(
              right: -4,
              top: -4,
              child: AnimatedSwitcher(
                duration: const Duration(milliseconds: 180),
                transitionBuilder: (c, a) =>
                    ScaleTransition(scale: a, child: c),
                child: Container(
                  key: ValueKey(count),
                  constraints:
                      const BoxConstraints(minWidth: 22, minHeight: 22),
                  padding: const EdgeInsets.symmetric(horizontal: 5),
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: Colors.white,
                    borderRadius: BorderRadius.circular(11),
                    border: Border.all(color: _C.bg, width: 2),
                  ),
                  child: Text('$count',
                      style: const TextStyle(
                          color: _C.accent,
                          fontSize: 12,
                          fontWeight: FontWeight.w700)),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Galereyaga ruxsat yo'q.
class _NoAccess extends StatelessWidget {
  final ScrollController scroll;
  const _NoAccess({required this.scroll});

  @override
  Widget build(BuildContext context) {
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
            style: FilledButton.styleFrom(backgroundColor: _C.accent),
            onPressed: PhotoManager.openSetting,
            child: const Text('Sozlamalarni ochish'),
          ),
        ),
      ],
    );
  }
}

/// Fayl bo'limidagi qator (`SharedDocumentCell`): 42 dp lik rangli
/// belgi, nomi (16) va izohi (13.5).
class _DocRow extends StatelessWidget {
  final IconData icon;
  final Color color;
  final String title;
  final String subtitle;
  final VoidCallback onTap;
  final bool divider;

  const _DocRow({
    required this.icon,
    required this.color,
    required this.title,
    required this.subtitle,
    required this.onTap,
    this.divider = true,
  });

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: SizedBox(
        height: 64,
        child: Stack(
          children: [
            Positioned.fill(
              child: Row(
                children: [
                  const SizedBox(width: 16),
                  Container(
                    width: 42,
                    height: 42,
                    decoration: BoxDecoration(
                      color: color,
                      borderRadius: BorderRadius.circular(21),
                    ),
                    child: Icon(icon, color: Colors.white, size: 22),
                  ),
                  const SizedBox(width: 14),
                  Expanded(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(title,
                            style: const TextStyle(
                                color: Colors.white, fontSize: 16)),
                        const SizedBox(height: 3),
                        Text(subtitle,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                                color: _C.hint, fontSize: 13.5)),
                      ],
                    ),
                  ),
                  const SizedBox(width: 16),
                ],
              ),
            ),
            if (divider)
              const Positioned(
                left: 72,
                right: 0,
                bottom: 0,
                child: SizedBox(
                    height: 0.6,
                    child: ColoredBox(color: Color(0x14FFFFFF))),
              ),
          ],
        ),
      ),
    );
  }
}

class _SectionShadow extends StatelessWidget {
  const _SectionShadow();

  @override
  Widget build(BuildContext context) =>
      Container(height: 10, color: Colors.black.withValues(alpha: 0.25));
}

/// Musiqa qatori (`SharedAudioCell`): dumaloq belgi, nomi, uzunligi,
/// o'ngda tanlash doirasi.
class _SongRow extends StatelessWidget {
  final AssetEntity song;
  final int? number;
  final VoidCallback onTap;

  const _SongRow(
      {required this.song, required this.number, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final s = song.duration;
    final dur = '${s ~/ 60}:${(s % 60).toString().padLeft(2, '0')}';
    return InkWell(
      onTap: onTap,
      child: SizedBox(
        height: 64,
        child: Row(
          children: [
            const SizedBox(width: 16),
            Container(
              width: 44,
              height: 44,
              decoration: const BoxDecoration(
                  color: _C.accent, shape: BoxShape.circle),
              child: const Icon(Icons.play_arrow_rounded,
                  color: Colors.white, size: 28),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(song.title ?? 'Nomsiz',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style:
                          const TextStyle(color: Colors.white, fontSize: 16)),
                  const SizedBox(height: 3),
                  Text(dur,
                      style: const TextStyle(color: _C.hint, fontSize: 13.5)),
                ],
              ),
            ),
            _CheckCircle(number: number, onDark: false),
            const SizedBox(width: 18),
          ],
        ),
      ),
    );
  }
}

/// `CheckBox2` (24 dp): bo'sh — oq halqa, tanlangan — urg'u rangida,
/// oq hoshiyali, ichida tartib raqami.
class _CheckCircle extends StatelessWidget {
  final int? number;

  /// Rasm ustida (bo'sh holatda ichi xira qora).
  final bool onDark;

  const _CheckCircle({required this.number, this.onDark = true});

  @override
  Widget build(BuildContext context) {
    final n = number;
    return AnimatedContainer(
      duration: const Duration(milliseconds: 180),
      width: 24,
      height: 24,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: n != null
            ? _C.accent
            : (onDark ? const Color(0x28000000) : Colors.transparent),
        border: Border.all(
            color: n != null || onDark ? Colors.white : _C.hint, width: 1.5),
      ),
      child: AnimatedScale(
        duration: const Duration(milliseconds: 180),
        scale: n == null ? 0 : 1,
        child: Text(n == null ? '' : '$n',
            style: const TextStyle(
                color: Colors.white,
                fontSize: 12,
                fontWeight: FontWeight.w700)),
      ),
    );
  }
}

/// Galereyadagi bitta katak (`PhotoAttachPhotoCell`).
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
    _thumb = _thumbs[widget.asset.id] ??= _load(widget.asset);
  }

  /// Ba'zi videolarda (skrinshotdagi qora kataklar) o'lchamli kichik
  /// rasm chiqmaydi — shunda standart kichik rasm, u ham bo'lmasa
  /// videoning birinchi kadri so'raladi.
  static Future<Uint8List?> _load(AssetEntity e) async {
    try {
      final a = await e.thumbnailDataWithSize(const ThumbnailSize.square(300),
          quality: 85);
      if (a != null && a.isNotEmpty) return a;
    } catch (_) {}
    try {
      final b = await e.thumbnailData;
      if (b != null && b.isNotEmpty) return b;
    } catch (_) {}
    try {
      return await e.thumbnailDataWithOption(ThumbnailOption(
          size: const ThumbnailSize.square(300),
          format: ThumbnailFormat.png,
          frame: 1));
    } catch (_) {
      return null;
    }
  }

  String _dur(int s) =>
      '${s ~/ 60}:${(s % 60).toString().padLeft(2, '0')}';

  @override
  Widget build(BuildContext context) {
    final e = widget.asset;
    final n = widget.number;
    return GestureDetector(
      onTap: widget.onTap,
      child: ColoredBox(
        color: const Color(0xFF0E0F11),
        child: Stack(
          fit: StackFit.expand,
          children: [
            AnimatedScale(
              scale: n != null ? 0.787 : 1,
              duration: const Duration(milliseconds: 200),
              curve: Curves.easeOut,
              child: Stack(
                fit: StackFit.expand,
                children: [
                  FutureBuilder<Uint8List?>(
                    future: _thumb,
                    builder: (_, s) => s.data == null
                        ? Container(color: Colors.white.withValues(alpha: 0.05))
                        : Image.memory(s.data!,
                            fit: BoxFit.cover, gaplessPlayback: true),
                  ),
                  if (e.type == AssetType.video)
                    Positioned(
                      left: 4,
                      bottom: 4,
                      height: 17,
                      child: Container(
                        padding: const EdgeInsets.symmetric(horizontal: 5),
                        decoration: BoxDecoration(
                          color: const Color(0x66000000),
                          borderRadius: BorderRadius.circular(4),
                        ),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            const Icon(Icons.play_arrow_rounded,
                                size: 12, color: Colors.white),
                            const SizedBox(width: 1),
                            Text(_dur(e.duration),
                                style: const TextStyle(
                                    color: Colors.white,
                                    fontSize: 12,
                                    height: 1.1,
                                    fontWeight: FontWeight.w600)),
                          ],
                        ),
                      ),
                    ),
                ],
              ),
            ),
            Positioned(
              right: 5,
              top: 5,
              child: _CheckCircle(number: n),
            ),
          ],
        ),
      ),
    );
  }
}
