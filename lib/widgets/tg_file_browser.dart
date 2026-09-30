// lib/widgets/tg_file_browser.dart — TELEGRAM'DAGIDEK FAYL TANLASH.
//
// Telegram `ChatAttachAlertDocumentLayout` / `DocumentSelectActivity`:
//   * "Ichki xotira" (jildlar bo'ylab yurish, ".." bilan yuqoriga),
//     "Galereya" qatorlari;
//   * "Oxirgi fayllar" — Download va boshqa odatiy jildlardagi eng
//     yangi fayllar;
//   * qidiruv, saralash (nom / sana / hajm), ko'p tanlash;
//   * fayl belgisi: kengaytmasi yozilgan rangli kvadrat, rasm bo'lsa
//     kichik ko'rinishi.
//
// Barcha jildlarni ko'rish uchun Android 11+ da "barcha fayllarga
// ruxsat" kerak (`aru/files` kanali, MainActivity). Ruxsat berilmasa —
// tizim tanlagichi (attach oynasidagi "Boshqa ilovalardan") qoladi.

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'glass.dart';

/// To'plam uchun ruxsat etilgan kengaytmalar (rasm va video).
const Set<String> kMediaExts = {
  'png',
  'jpg',
  'jpeg',
  'gif',
  'webp',
  'mp4',
  'webm',
  'mov',
  'mkv',
  '3gp',
};

const Set<String> kVideoExts = {'mp4', 'webm', 'mov', 'mkv', '3gp'};

class TgFileEntry {
  final File file;
  final String name;
  final int size;
  final DateTime modified;
  const TgFileEntry(this.file, this.name, this.size, this.modified);
}

abstract final class TgFiles {
  static const _ch = MethodChannel('aru/files');
  static const root = '/storage/emulated/0';

  /// Barcha jildlarga ruxsat bormi (kanal yo'q bo'lsa — bor deb olinadi).
  static Future<bool> hasAccess() async {
    try {
      return await _ch.invokeMethod<bool>('hasAll') ?? true;
    } catch (_) {
      return true;
    }
  }

  static Future<void> requestAccess() async {
    try {
      await _ch.invokeMethod<void>('requestAll');
    } catch (_) {}
  }

  static String ext(String name) {
    final i = name.lastIndexOf('.');
    return i < 0 ? '' : name.substring(i + 1).toLowerCase();
  }

  static bool allowed(String name, Set<String>? exts) =>
      exts == null || exts.contains(ext(name));

  static String baseName(String path) => path.split('/').last;

  static String size(int b) {
    if (b < 1024) return '$b B';
    if (b < 1048576) return '${(b / 1024).toStringAsFixed(1)} KB';
    if (b < 1073741824) return '${(b / 1048576).toStringAsFixed(1)} MB';
    return '${(b / 1073741824).toStringAsFixed(1)} GB';
  }

  static String date(DateTime d) {
    const m = [
      'yan',
      'fev',
      'mar',
      'apr',
      'may',
      'iyn',
      'iyl',
      'avg',
      'sen',
      'okt',
      'noy',
      'dek',
    ];
    final now = DateTime.now();
    final hm =
        '${d.hour.toString().padLeft(2, '0')}:${d.minute.toString().padLeft(2, '0')}';
    if (d.year == now.year && d.month == now.month && d.day == now.day) {
      return 'bugun, $hm';
    }
    return '${d.day} ${m[d.month - 1]}${d.year == now.year ? '' : ' ${d.year}'}, $hm';
  }

  /// Odatiy jildlar: Telegram "Oxirgi fayllar" uchun MediaStore'dan oladi,
  /// biz esa shu jildlarni ko'zdan kechiramiz (chuqurlashmasdan).
  static const List<String> _recentDirs = [
    '$root/Download',
    '$root/Documents',
    '$root/DCIM/Camera',
    '$root/Pictures',
    '$root/Pictures/Screenshots',
    '$root/Movies',
    '$root/Telegram/Telegram Video',
    '$root/Telegram/Telegram Images',
    '$root/Telegram/Telegram Documents',
  ];

  static Future<List<TgFileEntry>> recent(Set<String>? exts,
      {int limit = 40}) async {
    final out = <TgFileEntry>[];
    for (final d in _recentDirs) {
      try {
        final dir = Directory(d);
        if (!await dir.exists()) continue;
        var n = 0;
        await for (final e in dir.list(followLinks: false)) {
          if (e is! File) continue;
          final name = baseName(e.path);
          if (name.startsWith('.') || !allowed(name, exts)) continue;
          final st = await e.stat();
          out.add(TgFileEntry(e, name, st.size, st.modified));
          if (++n >= 300) break;
        }
      } catch (_) {}
    }
    out.sort((a, b) => b.modified.compareTo(a.modified));
    return out.length > limit ? out.sublist(0, limit) : out;
  }
}

/// Fayl belgisi (`SharedDocumentCell`): kengaytmali rangli kvadrat,
/// rasm bo'lsa kichik ko'rinishi.
class TgFileIcon extends StatelessWidget {
  final String name;
  final File? file;
  const TgFileIcon(this.name, {super.key, this.file});

  static Color colorOf(String ext) => switch (ext) {
        'apk' || 'enc' || 'xlsx' || 'csv' => const Color(0xFF4FA35A),
        'zip' ||
        'rar' ||
        '7z' ||
        'tar' ||
        'gz' ||
        'session' =>
          const Color(0xFFD9A93B),
        'pdf' || 'ppt' || 'pptx' => const Color(0xFFD8524B),
        'doc' || 'docx' || 'txt' || 'json' || 'xml' => const Color(0xFF4C90D9),
        'mp3' || 'm4a' || 'flac' || 'wav' || 'ogg' => const Color(0xFFE2620F),
        _ => const Color(0xFF6F7780),
      };

  @override
  Widget build(BuildContext context) {
    final ext = TgFiles.ext(name);
    final isImg = const {'png', 'jpg', 'jpeg', 'webp', 'gif'}.contains(ext);
    final f = file;
    Widget box() => Container(
          width: 44,
          height: 44,
          decoration: BoxDecoration(
            color: colorOf(ext),
            borderRadius: BorderRadius.circular(8),
          ),
          alignment: Alignment.center,
          child: Text(
            ext.isEmpty ? 'fayl' : (ext.length > 4 ? ext.substring(0, 4) : ext),
            style: const TextStyle(
                color: Colors.white, fontSize: 12, fontWeight: FontWeight.w700),
          ),
        );
    if (isImg && f != null) {
      return ClipRRect(
        borderRadius: BorderRadius.circular(8),
        child: Image.file(f,
            width: 44,
            height: 44,
            fit: BoxFit.cover,
            cacheWidth: 132,
            errorBuilder: (_, __, ___) => box()),
      );
    }
    return box();
  }
}

/// Bitta qator: belgi + nom + "hajm · sana" + tanlash belgisi.
class TgFileRow extends StatelessWidget {
  final Widget leading;
  final String title;
  final String subtitle;
  final bool selected;
  final bool showCheck;
  final VoidCallback onTap;
  final bool divider;

  const TgFileRow({
    super.key,
    required this.leading,
    required this.title,
    required this.subtitle,
    required this.onTap,
    this.selected = false,
    this.showCheck = false,
    this.divider = true,
  });

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Container(
        constraints: const BoxConstraints(minHeight: 66),
        padding: const EdgeInsets.fromLTRB(14, 8, 14, 8),
        decoration: BoxDecoration(
          border: divider
              ? Border(
                  bottom: BorderSide(
                      color: Colors.white.withValues(alpha: 0.06), width: 0.7))
              : null,
        ),
        child: Row(
          children: [
            leading,
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style:
                          const TextStyle(color: Colors.white, fontSize: 16)),
                  const SizedBox(height: 3),
                  Text(subtitle,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          color: Color(0xFF8A939D), fontSize: 13.5)),
                ],
              ),
            ),
            if (showCheck)
              Container(
                width: 24,
                height: 24,
                margin: const EdgeInsets.only(left: 8),
                decoration: BoxDecoration(
                  color: selected ? AppColors.accent2 : Colors.transparent,
                  shape: BoxShape.circle,
                  border: Border.all(
                      color: selected ? AppColors.accent2 : Colors.white54,
                      width: 1.8),
                ),
                child: selected
                    ? const Icon(Icons.check_rounded,
                        color: Colors.white, size: 16)
                    : null,
              ),
          ],
        ),
      ),
    );
  }
}

enum _Sort { name, date, size }

/// To'liq ekran: jildlar bo'ylab yurib fayl(lar) tanlash.
/// Natija — tanlangan fayllar (yoki `null`).
class TgFileBrowserScreen extends StatefulWidget {
  final Set<String>? exts;
  final int maxPick;
  const TgFileBrowserScreen({super.key, this.exts, this.maxPick = 10});

  @override
  State<TgFileBrowserScreen> createState() => _TgFileBrowserScreenState();
}

class _TgFileBrowserScreenState extends State<TgFileBrowserScreen>
    with WidgetsBindingObserver {
  Directory _dir = Directory(TgFiles.root);
  List<Directory> _dirs = const [];
  List<TgFileEntry> _files = const [];
  bool _loading = true;
  bool _access = true;
  String? _error;
  bool _searching = false;
  final _q = TextEditingController();
  _Sort _sort = _Sort.name;
  final List<File> _sel = [];

  bool get _atRoot => _dir.path == TgFiles.root;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    unawaited(_open(_dir));
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _q.dispose();
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState s) {
    // Sozlamalardan qaytganda ruxsat qayta tekshiriladi.
    if (s == AppLifecycleState.resumed && !_access) unawaited(_open(_dir));
  }

  Future<void> _open(Directory d) async {
    setState(() {
      _loading = true;
      _error = null;
    });
    final ok = await TgFiles.hasAccess();
    if (!mounted) return;
    if (!ok) {
      setState(() {
        _access = false;
        _loading = false;
      });
      return;
    }
    final dirs = <Directory>[];
    final files = <TgFileEntry>[];
    String? err;
    try {
      final list = <FileSystemEntity>[];
      await for (final e in d.list(followLinks: false)) {
        list.add(e);
      }
      final stats = <File, Future<FileStat>>{};
      for (final e in list) {
        final name = TgFiles.baseName(e.path);
        if (name.startsWith('.')) continue;
        if (e is Directory) {
          dirs.add(e);
        } else if (e is File && TgFiles.allowed(name, widget.exts)) {
          stats[e] = e.stat();
        }
      }
      for (final e in stats.entries) {
        final st = await e.value;
        files.add(TgFileEntry(
            e.key, TgFiles.baseName(e.key.path), st.size, st.modified));
      }
    } catch (e) {
      final t = '$e'.replaceAll(RegExp(r'\s+'), ' ');
      err = 'Bu jildni ochib bo\'lmadi: ${t.length > 110 ? t.substring(0, 110) : t}';
    }
    if (!mounted) return;
    // Bosh jildni ham o'qib bo'lmasa — ruxsat yo'q: tugma ko'rsatiladi.
    if (err != null && d.path == TgFiles.root) {
      setState(() {
        _dir = d;
        _access = false;
        _error = err;
        _loading = false;
      });
      return;
    }
    setState(() {
      _dir = d;
      _dirs = dirs;
      _files = files;
      _access = true;
      _error = err;
      _loading = false;
      _searching = false;
      _q.clear();
    });
  }

  void _up() {
    if (_atRoot) {
      Navigator.of(context).pop();
      return;
    }
    unawaited(_open(_dir.parent));
  }

  void _toggle(File f) {
    setState(() {
      if (!_sel.any((x) => x.path == f.path)) {
        if (_sel.length < widget.maxPick) _sel.add(f);
      } else {
        _sel.removeWhere((x) => x.path == f.path);
      }
    });
  }

  Widget _round(IconData i, VoidCallback onTap) => GestureDetector(
        onTap: onTap,
        child: Container(
          width: 48,
          height: 48,
          decoration: const BoxDecoration(
              color: Color(0xFF1C1C1E), shape: BoxShape.circle),
          child: Icon(i, color: Colors.white, size: 24),
        ),
      );

  Widget _header() {
    final title = _atRoot ? 'Ichki xotira' : TgFiles.baseName(_dir.path);
    return Padding(
      padding: const EdgeInsets.fromLTRB(10, 6, 10, 6),
      child: Row(
        children: [
          _round(Icons.arrow_back_rounded, _up),
          const SizedBox(width: 8),
          Expanded(
            child: Container(
              height: 48,
              padding: const EdgeInsets.symmetric(horizontal: 18),
              decoration: BoxDecoration(
                color: const Color(0xFF1C1C1E),
                borderRadius: BorderRadius.circular(24),
              ),
              alignment: Alignment.centerLeft,
              child: _searching
                  ? TextField(
                      controller: _q,
                      autofocus: true,
                      onChanged: (_) => setState(() {}),
                      style: const TextStyle(color: Colors.white, fontSize: 17),
                      cursorColor: AppColors.accent2,
                      decoration: const InputDecoration(
                        border: InputBorder.none,
                        isDense: true,
                        hintText: 'Qidirish',
                        hintStyle: TextStyle(color: Color(0xFF8A939D)),
                      ),
                    )
                  : Text(title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          color: Colors.white,
                          fontSize: 19,
                          fontWeight: FontWeight.w600)),
            ),
          ),
          const SizedBox(width: 8),
          _round(_searching ? Icons.close_rounded : Icons.search_rounded, () {
            setState(() {
              _searching = !_searching;
              if (!_searching) _q.clear();
            });
          }),
          const SizedBox(width: 8),
          PopupMenuButton<_Sort>(
            color: AppColors.cardAlt,
            onSelected: (v) => setState(() => _sort = v),
            shape:
                RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
            itemBuilder: (_) => [
              for (final e in const {
                _Sort.name: 'Nomi bo\'yicha',
                _Sort.date: 'Sanasi bo\'yicha',
                _Sort.size: 'Hajmi bo\'yicha',
              }.entries)
                PopupMenuItem(
                  value: e.key,
                  child: Text(e.value,
                      style: TextStyle(
                          color: _sort == e.key
                              ? AppColors.accent2
                              : Colors.white)),
                ),
            ],
            child: Container(
              width: 48,
              height: 48,
              decoration: const BoxDecoration(
                  color: Color(0xFF1C1C1E), shape: BoxShape.circle),
              child:
                  const Icon(Icons.sort_rounded, color: Colors.white, size: 24),
            ),
          ),
        ],
      ),
    );
  }

  Widget _card(List<Widget> rows) => Container(
        margin: const EdgeInsets.fromLTRB(12, 6, 12, 6),
        decoration: BoxDecoration(
          color: const Color(0xFF17171A),
          borderRadius: BorderRadius.circular(20),
        ),
        clipBehavior: Clip.antiAlias,
        child: Column(children: rows),
      );

  Widget _folderIcon() => Container(
        width: 44,
        height: 44,
        decoration: const BoxDecoration(
            color: Color(0xFFB8643A), shape: BoxShape.circle),
        child:
            const Icon(Icons.folder_rounded, color: Colors.white70, size: 24),
      );

  Widget _noAccess() => Padding(
        padding: const EdgeInsets.all(28),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.folder_off_rounded,
                color: Colors.white38, size: 56),
            const SizedBox(height: 14),
            const Text(
              'Jildlarni ko\'rish uchun "barcha fayllarga ruxsat" kerak.',
              textAlign: TextAlign.center,
              style: TextStyle(color: Colors.white70, fontSize: 15.5),
            ),
            if (_error != null) ...[
              const SizedBox(height: 8),
              Text(_error!,
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                      color: Color(0xFF8A939D), fontSize: 12.5)),
            ],
            const SizedBox(height: 18),
            FilledButton(
              style: FilledButton.styleFrom(backgroundColor: AppColors.accent),
              onPressed: TgFiles.requestAccess,
              child: const Text('Ruxsat berish'),
            ),
          ],
        ),
      );

  @override
  Widget build(BuildContext context) {
    final q = _q.text.trim().toLowerCase();
    final dirs = [
      for (final d in _dirs)
        if (q.isEmpty || TgFiles.baseName(d.path).toLowerCase().contains(q)) d
    ]..sort((a, b) => TgFiles.baseName(a.path)
        .toLowerCase()
        .compareTo(TgFiles.baseName(b.path).toLowerCase()));
    final files = [
      for (final f in _files)
        if (q.isEmpty || f.name.toLowerCase().contains(q)) f
    ]..sort((a, b) => switch (_sort) {
          _Sort.name => a.name.toLowerCase().compareTo(b.name.toLowerCase()),
          _Sort.date => b.modified.compareTo(a.modified),
          _Sort.size => b.size.compareTo(a.size),
        });
    return PopScope(
      canPop: _atRoot,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) _up();
      },
      child: Scaffold(
        backgroundColor: AppColors.bg,
        body: SafeArea(
          child: Stack(
            children: [
              Column(
                children: [
                  _header(),
                  Expanded(
                    child: _loading
                        ? const Center(
                            child: CircularProgressIndicator(
                                strokeWidth: 2.4, color: Colors.white54))
                        : !_access
                            ? _noAccess()
                            : ListView(
                                padding: const EdgeInsets.only(bottom: 110),
                                children: [
                                  if (_error != null)
                                    Padding(
                                      padding: const EdgeInsets.all(24),
                                      child: Text(_error!,
                                          textAlign: TextAlign.center,
                                          style: const TextStyle(
                                              color: AppColors.danger)),
                                    ),
                                  _card([
                                    TgFileRow(
                                      leading: _folderIcon(),
                                      title: '..',
                                      subtitle:
                                          _atRoot ? 'Yopish' : _dir.parent.path,
                                      onTap: _up,
                                      divider:
                                          dirs.isNotEmpty || files.isNotEmpty,
                                    ),
                                    for (var i = 0; i < dirs.length; i++)
                                      TgFileRow(
                                        leading: _folderIcon(),
                                        title: TgFiles.baseName(dirs[i].path),
                                        subtitle: 'Jild',
                                        onTap: () => _open(dirs[i]),
                                        divider: i < dirs.length - 1 ||
                                            files.isNotEmpty,
                                      ),
                                    for (var i = 0; i < files.length; i++)
                                      TgFileRow(
                                        leading: TgFileIcon(files[i].name,
                                            file: files[i].file),
                                        title: files[i].name,
                                        subtitle:
                                            '${TgFiles.size(files[i].size)}, '
                                            '${TgFiles.date(files[i].modified)}',
                                        showCheck: true,
                                        selected: _sel.any((x) =>
                                            x.path == files[i].file.path),
                                        onTap: () => _toggle(files[i].file),
                                        divider: i < files.length - 1,
                                      ),
                                  ]),
                                  if (dirs.isEmpty &&
                                      files.isEmpty &&
                                      _error == null)
                                    const Padding(
                                      padding: EdgeInsets.all(24),
                                      child: Text('Bu yerda hech narsa yo\'q',
                                          textAlign: TextAlign.center,
                                          style: TextStyle(
                                              color: Color(0xFF8A939D))),
                                    ),
                                ],
                              ),
                  ),
                ],
              ),
              if (_sel.isNotEmpty)
                Positioned(
                  left: 12,
                  right: 12,
                  bottom: 12,
                  child: Row(
                    children: [
                      Expanded(
                        child: Container(
                          height: 52,
                          padding: const EdgeInsets.symmetric(horizontal: 20),
                          alignment: Alignment.centerLeft,
                          decoration: BoxDecoration(
                            color: const Color(0xFF26292E),
                            borderRadius: BorderRadius.circular(26),
                          ),
                          child: Text('${_sel.length} ta tanlandi',
                              style: const TextStyle(
                                  color: Colors.white, fontSize: 16)),
                        ),
                      ),
                      const SizedBox(width: 8),
                      GestureDetector(
                        onTap: () => Navigator.of(context).pop(_sel),
                        child: Container(
                          width: 56,
                          height: 56,
                          decoration: const BoxDecoration(
                              color: AppColors.accent, shape: BoxShape.circle),
                          child: const Icon(Icons.send_rounded,
                              color: Colors.white, size: 26),
                        ),
                      ),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
