// lib/screens/pack_detail_screen.dart — BITTA TO'PLAM.
//
// Egasi: element qo'shadi (admin ko'rib chiqadi), olib tashlaydi, to'plamni
// o'chiradi; kutayotgan va rad etilgan rasmlar sababi bilan ko'rinadi.
// Boshqalar: to'plamni ko'radi va qo'shadi / olib tashlaydi.
//
// Elementlar ro'yxatida faqat kichik statik rasmlar (telefonga bosim
// tushmasin); bosilganda element o'z animatsiyasi bilan ochiladi.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';

import '../services/auth_service.dart';
import '../services/pack_service.dart';
import '../services/storage_janitor.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../widgets/pack_views.dart';

class PackDetailScreen extends StatefulWidget {
  final int packId;
  final PackInfo? initial;
  const PackDetailScreen({super.key, required this.packId, this.initial});

  @override
  State<PackDetailScreen> createState() => _PackDetailScreenState();
}

class _PackDetailScreenState extends State<PackDetailScreen> {
  final _svc = PackService.instance;
  PackHeader? _header;
  PackInfo? _fetched;
  bool _uploading = false;
  String _progress = '';

  @override
  void initState() {
    super.initState();
    _svc.addListener(_onSvc);
    unawaited(_init());
  }

  @override
  void dispose() {
    _svc.removeListener(_onSvc);
    super.dispose();
  }

  void _onSvc() {
    if (!mounted) return;
    setState(() {});
    unawaited(_loadHeader());
  }

  Future<void> _init() async {
    await _svc.load();
    if (_current() == null) {
      _fetched = await _svc.infoFor(widget.packId);
    }
    await _loadHeader();
    if (mounted) setState(() {});
  }

  PackInfo? _current() {
    for (final p in [..._svc.myPacks, ..._svc.subPacks]) {
      if (p.id == widget.packId) return p;
    }
    return _fetched ?? widget.initial;
  }

  bool get _isMine {
    final me = AuthService.instance.user?.id ?? 0;
    final p = _current();
    return p != null && me > 0 && p.ownerId == me;
  }

  Future<void> _loadHeader() async {
    final p = _current();
    if (p == null || !p.usable) return;
    if (_header != null && _header!.ver >= p.version) return;
    final h = await _svc.header(p);
    if (mounted && h != null) setState(() => _header = h);
  }

  void _say(String t) => ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          behavior: SnackBarBehavior.floating,
          backgroundColor: AppColors.card,
          content: Text(t, style: const TextStyle(color: Colors.white)),
        ),
      );

  Future<bool> _confirm(String text, String yes) async {
    final r = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.card,
        content: Text(text,
            style: const TextStyle(color: Colors.white, height: 1.4)),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: const Text('Bekor')),
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(true),
              child: Text(yes,
                  style: const TextStyle(color: AppColors.danger))),
        ],
      ),
    );
    return r == true;
  }

  // ── ELEMENT QO'SHISH ─────────────────────────────────────────

  Future<void> _add() async {
    final p = _current();
    if (p == null || _uploading) return;
    List<XFile> picked;
    try {
      picked = await ImagePicker().pickMultiImage(limit: 20);
    } catch (_) {
      _say('Rasm tanlab bo\'lmadi');
      return;
    }
    if (picked.isEmpty || !mounted) return;

    // Bitta rasm bo'lsa — unga mos emoji so'raladi (ixtiyoriy).
    var emoji = '';
    if (picked.length == 1 && p.kind != PackKind.gif) {
      final ctl = TextEditingController();
      final r = await showDialog<String>(
        context: context,
        builder: (ctx) => AlertDialog(
          backgroundColor: AppColors.card,
          title: const Text('Mos emoji (ixtiyoriy)',
              style: TextStyle(color: Colors.white, fontSize: 16)),
          content: TextField(
            controller: ctl,
            autofocus: true,
            style: const TextStyle(color: Colors.white, fontSize: 22),
            decoration: const InputDecoration(
              hintText: '😀',
              hintStyle: TextStyle(color: Colors.white24),
            ),
          ),
          actions: [
            TextButton(
                onPressed: () => Navigator.of(ctx).pop(''),
                child: const Text('O\'tkazish')),
            TextButton(
                onPressed: () => Navigator.of(ctx).pop(ctl.text),
                child: const Text('Yuklash')),
          ],
        ),
      );
      if (r == null || !mounted) {
        unawaited(StorageJanitor.dropPicked(picked.first.path));
        return;
      }
      emoji = r;
    }

    setState(() => _uploading = true);
    var ok = 0;
    String? firstError;
    for (var i = 0; i < picked.length; i++) {
      if (!mounted) return;
      setState(() => _progress = '${i + 1} / ${picked.length}');
      final err = await _svc.addItem(
        p,
        picked[i].path,
        emoji: emoji,
        onProgress: (sent, total) {
          if (!mounted || total <= 0) return;
          setState(() => _progress =
              '${i + 1} / ${picked.length} · ${(sent * 100 / total).floor()}%');
        },
      );
      unawaited(StorageJanitor.dropPicked(picked[i].path));
      if (err == null) {
        ok++;
      } else {
        firstError ??= err;
      }
    }
    if (!mounted) return;
    setState(() {
      _uploading = false;
      _progress = '';
    });
    if (ok > 0) {
      _say(ok == picked.length
          ? '$ok ta yuborildi — admin ko\'rib chiqadi'
          : '$ok ta yuborildi. Qolganlari: ${firstError ?? 'xato'}');
    } else {
      _say(firstError ?? 'Yuborilmadi');
    }
  }

  // ── ELEMENTNI KO'RISH / O'CHIRISH ────────────────────────────

  Future<void> _open(PackInfo p, PackItem it) async {
    final mine = _isMine;
    final act = await showDialog<String>(
      context: context,
      barrierColor: Colors.black87,
      builder: (ctx) => Dialog(
        backgroundColor: Colors.transparent,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            PackImage(pack: p.id, item: it.id, size: 240, animate: true),
            const SizedBox(height: 10),
            if (it.emoji.isNotEmpty)
              Text(it.emoji, style: const TextStyle(fontSize: 26)),
            const SizedBox(height: 10),
            if (mine)
              TextButton.icon(
                onPressed: () => Navigator.of(ctx).pop('remove'),
                icon: const Icon(Icons.delete_outline_rounded,
                    color: AppColors.danger),
                label: const Text('To\'plamdan olib tashlash',
                    style: TextStyle(color: AppColors.danger)),
              ),
          ],
        ),
      ),
    );
    if (act == 'remove' && mounted) {
      if (await _confirm('Bu element to\'plamdan olib tashlansinmi?',
          'Olib tashlash')) {
        _svc.removeItem(p.id, it.id);
      }
    }
  }

  Future<void> _deletePack(PackInfo p, {bool admin = false}) async {
    if (!await _confirm(
        'To\'plam butunlay o\'chirilsinmi? Uni qo\'shgan foydalanuvchilarda '
        'ham yo\'qoladi.',
        'O\'chirish')) {
      return;
    }
    if (admin) {
      final err = await _svc.adminDeletePack(p.id);
      if (err != null) {
        _say(err);
        return;
      }
    } else {
      _svc.deletePack(p.id);
    }
    if (mounted) Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final p = _current();
    if (p == null) {
      return AppBackground(
        child: Scaffold(
          backgroundColor: Colors.transparent,
          appBar: AppBar(
              backgroundColor: Colors.transparent,
              iconTheme: const IconThemeData(color: Colors.white)),
          body: const Center(
              child: Text('To\'plam topilmadi',
                  style: TextStyle(color: Colors.white60))),
        ),
      );
    }
    final mine = _isMine;
    final admin = AuthService.instance.user?.isAdmin == true;
    final gone = _svc.removedItems(p.id);
    final items = [
      for (final it in _header?.items ?? const <PackItem>[])
        if (!gone.contains(it.id)) it,
    ];
    final ops = mine ? _svc.opsOf(p.id).where((o) => o.isAdd).toList() : const <PackOp>[];
    final cols = p.kind == PackKind.gif ? 3 : 4;
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          title: Text(p.title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(color: Colors.white, fontSize: 18)),
          actions: [
            if (mine || admin)
              PopupMenuButton<String>(
                color: AppColors.card,
                iconColor: Colors.white,
                onSelected: (v) => _deletePack(p, admin: !mine),
                itemBuilder: (_) => [
                  PopupMenuItem(
                    value: 'delete',
                    child: Text(
                        mine ? 'To\'plamni o\'chirish' : 'To\'plamni o\'chirish (admin)',
                        style: const TextStyle(color: AppColors.danger)),
                  ),
                ],
              ),
          ],
        ),
        floatingActionButton: mine
            ? FloatingActionButton.extended(
                backgroundColor: AppColors.accent,
                foregroundColor: Colors.white,
                onPressed: _uploading ? null : _add,
                icon: _uploading
                    ? const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(
                            strokeWidth: 2, color: Colors.white))
                    : const Icon(Icons.add_photo_alternate_rounded),
                label: Text(_uploading ? _progress : 'Rasm qo\'shish'),
              )
            : null,
        body: ListView(
          padding: const EdgeInsets.fromLTRB(16, 4, 16, 110),
          children: [
            Text(
              '${PackKind.single(p.kind)} · ${p.items} ta'
              '${p.ownerName.isEmpty ? '' : ' · ${p.ownerName}'}',
              style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.55), fontSize: 13),
            ),
            if (!mine && p.ownerId != 0) ...[
              const SizedBox(height: 12),
              _subBar(p),
            ],
            const SizedBox(height: 14),
            if (ops.isNotEmpty) ...[
              _opsCard(ops),
              const SizedBox(height: 14),
            ],
            if (!p.usable && ops.isEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 30),
                child: Center(
                  child: Text(
                    mine
                        ? 'To\'plam bo\'sh. "Rasm qo\'shish" tugmasi bilan '
                            'rasm yuboring — admin tasdiqlagach shu yerda '
                            'ko\'rinadi.'
                        : 'To\'plam hali bo\'sh.',
                    textAlign: TextAlign.center,
                    style: TextStyle(
                        color: Colors.white.withValues(alpha: 0.5),
                        height: 1.5),
                  ),
                ),
              )
            else if (p.usable && _header == null)
              const Padding(
                padding: EdgeInsets.only(top: 40),
                child: Center(
                    child: CircularProgressIndicator(
                        strokeWidth: 2.4, color: Colors.white54)),
              )
            else
              GridView.builder(
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                itemCount: items.length,
                gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: cols,
                  mainAxisSpacing: 8,
                  crossAxisSpacing: 8,
                ),
                itemBuilder: (ctx, i) {
                  final it = items[i];
                  return GestureDetector(
                    onTap: () => _open(p, it),
                    child: Container(
                      decoration: BoxDecoration(
                        color: Colors.white.withValues(alpha: 0.05),
                        borderRadius: BorderRadius.circular(14),
                      ),
                      padding: const EdgeInsets.all(6),
                      child: LayoutBuilder(
                        builder: (_, box) => PackImage(
                            pack: p.id, item: it.id, size: box.maxWidth),
                      ),
                    ),
                  );
                },
              ),
          ],
        ),
      ),
    );
  }

  Widget _subBar(PackInfo p) {
    final on = _svc.isSubscribed(p.id);
    return GestureDetector(
      onTap: () => _svc.setSubscribed(p, !on),
      child: Container(
        padding: const EdgeInsets.symmetric(vertical: 13),
        decoration: BoxDecoration(
          color: on ? Colors.white.withValues(alpha: 0.08) : AppColors.accent,
          borderRadius: BorderRadius.circular(14),
        ),
        child: Center(
          child: Text(
            on ? 'Qo\'shilgan — olib tashlash' : 'To\'plamni qo\'shish',
            style: const TextStyle(
                color: Colors.white,
                fontSize: 14.5,
                fontWeight: FontWeight.w600),
          ),
        ),
      ),
    );
  }

  Widget _opsCard(List<PackOp> ops) {
    String label(PackOp o) => switch (o.state) {
          'queued' => 'Yuborilmoqda...',
          'pending' => 'Admin ko\'rib chiqmoqda',
          'approved' => 'Tasdiqlandi — to\'plamga qo\'shilmoqda',
          'rejected' => 'Rad etildi: ${o.reason.isEmpty ? 'sabab ko\'rsatilmagan' : o.reason}',
          _ => o.state,
        };
    Color color(PackOp o) => switch (o.state) {
          'rejected' => AppColors.danger,
          'approved' => AppColors.success,
          _ => AppColors.gold,
        };
    return Glass(
      borderRadius: 18,
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Yuborilgan rasmlar',
              style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.7),
                  fontSize: 13,
                  fontWeight: FontWeight.w700)),
          const SizedBox(height: 8),
          for (final o in ops)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: Row(
                children: [
                  Container(
                      width: 8,
                      height: 8,
                      decoration:
                          BoxDecoration(color: color(o), shape: BoxShape.circle)),
                  const SizedBox(width: 10),
                  if (o.emoji.isNotEmpty) ...[
                    Text(o.emoji, style: const TextStyle(fontSize: 16)),
                    const SizedBox(width: 6),
                  ],
                  Expanded(
                    child: Text(label(o),
                        style: TextStyle(
                            color: Colors.white.withValues(alpha: 0.8),
                            fontSize: 13,
                            height: 1.35)),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}
