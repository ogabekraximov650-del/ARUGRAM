// lib/screens/admin_packs_screen.dart — ADMIN: TO'PLAMGA QO'SHILAYOTGAN
// RASMLARNI KO'RIB CHIQISH.
//
// Foydalanuvchi yuklagan har bir emoji, GIF va stiker avval SHU YERDA
// tekshiriladi. Tasdiqlansa Actions uni to'plamga qo'shadi (yengil
// WebP ga aylantirib), rad etilsa sabab bilan egasiga qaytadi va fayl
// kanaldan o'chadi.
//
// Rasm foydalanuvchi yuborgan ASL faylning o'zi (shifrlangan, kanalda):
// admin uni o'z Telegram hisobi orqali ochadi (`TelegramService`).

import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../services/pack_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';

class AdminPacksScreen extends StatefulWidget {
  const AdminPacksScreen({super.key});

  @override
  State<AdminPacksScreen> createState() => _AdminPacksScreenState();
}

class _AdminPacksScreenState extends State<AdminPacksScreen> {
  final _svc = PackService.instance;
  List<Map<String, dynamic>>? _ops;
  bool _loading = true;
  final Set<int> _busy = {};
  final Map<String, Future<Uint8List?>> _images = {};

  static const _reasons = [
    'Nomaqbul kontent',
    'Sifati past',
    'Mualliflik huquqi buzilgan',
    'Noto\'g\'ri to\'plamga yuborilgan',
  ];

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    final r = await _svc.adminPending();
    if (!mounted) return;
    setState(() {
      _ops = r;
      _loading = false;
    });
  }

  int _id(Map<String, dynamic> o) => (o['id'] as num?)?.toInt() ?? 0;

  void _say(String t) => ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          behavior: SnackBarBehavior.floating,
          backgroundColor: AppColors.card,
          content: Text(t, style: const TextStyle(color: Colors.white)),
        ),
      );

  Future<void> _review(Map<String, dynamic> o, bool approve,
      {String reason = ''}) async {
    final id = _id(o);
    if (id <= 0 || _busy.contains(id)) return;
    setState(() => _busy.add(id));
    final err = await _svc.adminReview([id], approve, reason: reason);
    if (!mounted) return;
    setState(() => _busy.remove(id));
    if (err != null) {
      _say(err);
      return;
    }
    setState(() => _ops?.removeWhere((e) => _id(e) == id));
  }

  Future<void> _reject(Map<String, dynamic> o) async {
    final ctl = TextEditingController();
    final reason = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.card,
        title: const Text('Nima uchun rad etiladi?',
            style: TextStyle(color: Colors.white, fontSize: 16)),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final r in _reasons)
                  ActionChip(
                    label: Text(r, style: const TextStyle(fontSize: 12.5)),
                    onPressed: () => Navigator.of(ctx).pop(r),
                  ),
              ],
            ),
            const SizedBox(height: 12),
            TextField(
              controller: ctl,
              maxLength: 120,
              style: const TextStyle(color: Colors.white),
              decoration: const InputDecoration(
                hintText: 'Yoki o\'zingiz yozing',
                hintStyle: TextStyle(color: Colors.white38),
              ),
            ),
          ],
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(),
              child: const Text('Bekor')),
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(ctl.text.trim()),
              child: const Text('Rad etish',
                  style: TextStyle(color: AppColors.danger))),
        ],
      ),
    );
    if (reason == null || !mounted) return;
    await _review(o, false, reason: reason);
  }

  Future<void> _approveAll() async {
    final ops = _ops;
    if (ops == null || ops.isEmpty) return;
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.card,
        content: Text('Hamma ${ops.length} ta rasm ko\'rib chiqilib, '
            'tasdiqlansinmi?',
            style: const TextStyle(color: Colors.white)),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: const Text('Yo\'q')),
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(true),
              child: const Text('Tasdiqlash')),
        ],
      ),
    );
    if (ok != true || !mounted) return;
    final ids = [for (final o in ops) _id(o)].where((i) => i > 0).toList();
    final err = await _svc.adminReview(ids, true);
    if (!mounted) return;
    if (err != null) {
      _say(err);
      return;
    }
    await _load();
  }

  Future<Uint8List?> _image(String file) =>
      _images.putIfAbsent(file, () => _svc.stagingBytes(file));

  @override
  Widget build(BuildContext context) {
    final ops = _ops;
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          title: const Text('To\'plam rasmlari',
              style: TextStyle(color: Colors.white, fontSize: 18)),
          actions: [
            if (ops != null && ops.length > 1)
              TextButton(
                  onPressed: _approveAll,
                  child: const Text('Hammasini tasdiqlash')),
          ],
        ),
        body: _body(ops),
      ),
    );
  }

  Widget _body(List<Map<String, dynamic>>? ops) {
    if (_loading) {
      return const Center(
          child: CircularProgressIndicator(
              strokeWidth: 2.4, color: Colors.white54));
    }
    if (ops == null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text('Ro\'yxatni olib bo\'lmadi',
                style: TextStyle(color: Colors.white60)),
            TextButton(onPressed: _load, child: const Text('Qayta urinish')),
          ],
        ),
      );
    }
    if (ops.isEmpty) {
      return const Center(
        child: Text('Ko\'rib chiqiladigan rasm yo\'q',
            style: TextStyle(color: Colors.white60)),
      );
    }
    return RefreshIndicator(
      onRefresh: _load,
      child: ListView.builder(
        physics: const BouncingScrollPhysics(
            parent: AlwaysScrollableScrollPhysics()),
        padding: const EdgeInsets.fromLTRB(16, 4, 16, 40),
        itemCount: ops.length,
        itemBuilder: (_, i) => _row(ops[i]),
      ),
    );
  }

  Widget _row(Map<String, dynamic> o) {
    final id = _id(o);
    final file = '${o['file'] ?? ''}';
    final busy = _busy.contains(id);
    final kind = '${o['kind'] ?? ''}';
    final size = (o['size'] as num?)?.toInt() ?? 0;
    return Padding(
      key: ValueKey(id),
      padding: const EdgeInsets.only(bottom: 14),
      child: Glass(
        borderRadius: 18,
        padding: const EdgeInsets.all(14),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Center(
              child: Container(
                constraints: const BoxConstraints(maxHeight: 260, minHeight: 120),
                width: double.infinity,
                decoration: BoxDecoration(
                  color: Colors.white.withValues(alpha: 0.05),
                  borderRadius: BorderRadius.circular(14),
                ),
                child: FutureBuilder<Uint8List?>(
                  future: _image(file),
                  builder: (_, snap) {
                    if (snap.connectionState != ConnectionState.done) {
                      return const SizedBox(
                          height: 120,
                          child: Center(
                              child: CircularProgressIndicator(
                                  strokeWidth: 2, color: Colors.white54)));
                    }
                    final b = snap.data;
                    if (b == null) {
                      return const SizedBox(
                        height: 120,
                        child: Center(
                          child: Text(
                            'Rasmni ochib bo\'lmadi\n(Telegram hisobi ulanganmi?)',
                            textAlign: TextAlign.center,
                            style: TextStyle(color: Colors.white54),
                          ),
                        ),
                      );
                    }
                    return ClipRRect(
                      borderRadius: BorderRadius.circular(14),
                      child: Image.memory(
                        b,
                        fit: BoxFit.contain,
                        cacheWidth: 600,
                        gaplessPlayback: true,
                        errorBuilder: (_, __, ___) => const SizedBox(
                          height: 120,
                          child: Center(
                              child: Text('Rasm buzuq',
                                  style: TextStyle(color: AppColors.danger))),
                        ),
                      ),
                    );
                  },
                ),
              ),
            ),
            const SizedBox(height: 10),
            Text(
              '${'${o['title'] ?? ''}'}  ·  ${PackKind.single(kind)}',
              style: const TextStyle(
                  color: Colors.white,
                  fontSize: 15,
                  fontWeight: FontWeight.w600),
            ),
            const SizedBox(height: 3),
            Text(
              '${'${o['owner'] ?? ''}'.isEmpty ? 'Foydalanuvchi #${o['owner_id']}' : o['owner']}'
              ' · ${(size / 1024).ceil()} KB'
              '${'${o['emoji'] ?? ''}'.isEmpty ? '' : ' · ${o['emoji']}'}',
              style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.55), fontSize: 12.5),
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: OutlinedButton(
                    onPressed: busy ? null : () => _reject(o),
                    style: OutlinedButton.styleFrom(
                        foregroundColor: AppColors.danger,
                        side: const BorderSide(color: AppColors.danger)),
                    child: const Text('Rad etish'),
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: FilledButton(
                    onPressed: busy ? null : () => _review(o, true),
                    style: FilledButton.styleFrom(
                        backgroundColor: AppColors.accent),
                    child: busy
                        ? const SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(
                                strokeWidth: 2, color: Colors.white))
                        : const Text('Tasdiqlash'),
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
