// lib/screens/pack_detail_screen.dart — BITTA TO'PLAM.
//
// Egasi: element qo'shadi (admin ko'rib chiqadi), olib tashlaydi, to'plamni
// o'chiradi; kutayotgan va rad etilgan rasmlar sababi bilan ko'rinadi.
// Boshqalar: to'plamni ko'radi va qo'shadi / olib tashlaydi.
//
// Elementlar ro'yxatida faqat kichik statik rasmlar (telefonga bosim
// tushmasin); bosilganda element o'z animatsiyasi bilan ochiladi.

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart' show XFile;

import '../services/auth_service.dart';
import '../services/pack_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../widgets/pack_views.dart';
import 'pack_add_screen.dart';

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
    if (p == null) return;
    // Rasm ham, video ham (Telegram'dagidek).
    final (picked, err) = await pickPackMedia(context, kind: p.kind);
    if (err != null) {
      _say('Fayl tanlab bo\'lmadi: $err');
      return;
    }
    if (picked.isEmpty || !mounted) return;
    // Qo'shish oynasi: nima yuborilayotgani ko'rinadi, emoji tanlanadi,
    // video kesiladi.
    final n = await Navigator.of(context).push<int>(MaterialPageRoute(
        builder: (_) => PackAddScreen(pack: p, files: picked)));
    if (n != null && n > 0 && mounted) {
      _say('$n ta yuborildi — admin ko\'rib chiqadi');
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
            PackImage(
                pack: p.id, item: it.id, size: 240, animate: true, sound: true),
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
                onPressed: _add,
                icon: const Icon(Icons.add_photo_alternate_rounded),
                label: const Text('Qo\'shish'),
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
              _opsCard(ops, p),
              const SizedBox(height: 14),
            ],
            if (!p.usable && ops.isEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 30),
                child: Center(
                  child: Text(
                    mine
                        ? 'To\'plam bo\'sh. "Qo\'shish" tugmasi bilan '
                            'rasm yoki video yuboring — admin tasdiqlagach shu yerda '
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
            else ...[
              if (_svc.lastError.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(bottom: 10),
                  child: Text('Oxirgi xato: ${_svc.lastError}',
                      style: TextStyle(
                          color: AppColors.danger.withValues(alpha: 0.9),
                          fontSize: 11.5)),
                ),
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

  /// Yuborilgan narsaning kichik ko'rinishi (bo'lmasa — bo'sh joy).
  Widget _sentThumb(PackOp o) {
    final path = _svc.sentThumbPath(o.file);
    if (path == null || !File(path).existsSync()) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(right: 10),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(8),
        child: Image.file(File(path),
            width: 44,
            height: 44,
            fit: BoxFit.cover,
            cacheWidth: 132,
            errorBuilder: (_, __, ___) => const SizedBox(width: 44, height: 44)),
      ),
    );
  }

  Future<void> _resend(PackOp o, PackInfo p) async {
    final src = _svc.sentSourcePath(o.file);
    if (src == null) return;
    final n = await Navigator.of(context).push<int>(MaterialPageRoute(
        builder: (_) => PackAddScreen(pack: p, files: [XFile(src)])));
    if (n != null && n > 0 && mounted) _svc.clearOp(o);
  }

  Widget _opsCard(List<PackOp> ops, PackInfo p) {
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
          Row(
            children: [
              Expanded(
                child: Text('Yuborilgan rasmlar',
                    style: TextStyle(
                        color: Colors.white.withValues(alpha: 0.7),
                        fontSize: 13,
                        fontWeight: FontWeight.w700)),
              ),
              if (ops.any((o) => o.state == 'rejected'))
                GestureDetector(
                  onTap: () => _svc.clearRejected(p.id),
                  child: const Padding(
                    padding: EdgeInsets.symmetric(vertical: 4),
                    child: Text('Rad etilganlarni tozalash',
                        style: TextStyle(
                            color: AppColors.danger,
                            fontSize: 12.5,
                            fontWeight: FontWeight.w600)),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 8),
          for (final o in ops)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Container(
                          width: 8,
                          height: 8,
                          decoration: BoxDecoration(
                              color: color(o), shape: BoxShape.circle)),
                      const SizedBox(width: 10),
                      _sentThumb(o),
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
                  if (o.state == 'rejected')
                    Padding(
                      padding: const EdgeInsets.only(left: 18, top: 2),
                      child: Wrap(
                        spacing: 6,
                        children: [
                          if (_svc.sentSourcePath(o.file) != null)
                            TextButton.icon(
                              onPressed: () => _resend(o, p),
                              icon: const Icon(Icons.refresh_rounded, size: 18),
                              label: const Text('Qayta yuborish'),
                            ),
                          TextButton.icon(
                            onPressed: () => _svc.clearOp(o),
                            icon: const Icon(Icons.delete_outline_rounded,
                                size: 18, color: AppColors.danger),
                            label: const Text('O\'chirish',
                                style: TextStyle(color: AppColors.danger)),
                          ),
                        ],
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
