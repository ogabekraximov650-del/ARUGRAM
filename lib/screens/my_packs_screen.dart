// lib/screens/my_packs_screen.dart — EMOJI, GIF VA STIKERLAR (PROFIL).
//
// Foydalanuvchi shu yerda o'z to'plamlarini yaratadi, boshqalarning
// ommaviy to'plamlarini qo'shadi. Panelda (`tg_composer.dart`) faqat
// SHU YERDA turganlar ko'rinadi.
//
// Qanday ishlashi — `services/pack_service.dart` boshidagi izohda.

import 'dart:async';

import 'package:flutter/material.dart';

import '../services/auth_service.dart';
import '../services/pack_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../widgets/pack_views.dart';
import 'pack_detail_screen.dart';

class MyPacksScreen extends StatefulWidget {
  /// Qaysi tur tanlangan holda ochilsin.
  final String initialKind;
  const MyPacksScreen({super.key, this.initialKind = PackKind.sticker});

  @override
  State<MyPacksScreen> createState() => _MyPacksScreenState();
}

class _MyPacksScreenState extends State<MyPacksScreen> {
  late String _kind = widget.initialKind;
  final _svc = PackService.instance;

  @override
  void initState() {
    super.initState();
    _svc.addListener(_onSvc);
    unawaited(_svc.load(force: true));
  }

  @override
  void dispose() {
    _svc.removeListener(_onSvc);
    super.dispose();
  }

  void _onSvc() {
    if (mounted) setState(() {});
  }

  Future<void> _create() async {
    if (_svc.myPacks.length >= kPackMaxPerUser) {
      _say('$kPackMaxPerUser tadan ko\'p to\'plam yaratib bo\'lmaydi');
      return;
    }
    final ctl = TextEditingController();
    final title = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.card,
        title: Text('Yangi ${PackKind.single(_kind).toLowerCase()} to\'plami',
            style: const TextStyle(color: Colors.white, fontSize: 17)),
        content: TextField(
          controller: ctl,
          autofocus: true,
          maxLength: kPackTitleMax,
          style: const TextStyle(color: Colors.white),
          decoration: const InputDecoration(
            hintText: 'To\'plam nomi',
            hintStyle: TextStyle(color: Colors.white38),
          ),
          onSubmitted: (v) => Navigator.of(ctx).pop(v),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(),
              child: const Text('Bekor')),
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(ctl.text),
              child: const Text('Yaratish')),
        ],
      ),
    );
    if (title == null || title.trim().isEmpty || !mounted) return;
    final p = _svc.createPack(_kind, title);
    if (p == null) {
      _say('Nom noto\'g\'ri');
      return;
    }
    if (!mounted) return;
    unawaited(Navigator.of(context).push(MaterialPageRoute<void>(
        builder: (_) => PackDetailScreen(packId: p.id, initial: p))));
  }

  void _say(String t) => ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          behavior: SnackBarBehavior.floating,
          backgroundColor: AppColors.card,
          content: Text(t, style: const TextStyle(color: Colors.white)),
        ),
      );

  @override
  Widget build(BuildContext context) {
    final mine = _svc.myPacks.where((p) => p.kind == _kind).toList();
    final subs = _svc.subPacks.where((p) => p.kind == _kind).toList();
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          title: const Text('Emoji, GIF va stikerlar',
              style: TextStyle(color: Colors.white, fontSize: 18)),
        ),
        floatingActionButton: AuthService.instance.isLoggedIn
            ? FloatingActionButton.extended(
                backgroundColor: AppColors.accent,
                foregroundColor: Colors.white,
                onPressed: _create,
                icon: const Icon(Icons.add_rounded),
                label: const Text('Yangi to\'plam'),
              )
            : null,
        body: RefreshIndicator(
          onRefresh: () => _svc.load(force: true),
          child: ListView(
            physics: const BouncingScrollPhysics(
                parent: AlwaysScrollableScrollPhysics()),
            padding: const EdgeInsets.fromLTRB(16, 4, 16, 110),
            children: [
              _kinds(),
              const SizedBox(height: 14),
              if (!AuthService.instance.isLoggedIn)
                _hint('To\'plam yaratish uchun hisobingizga kiring.')
              else ...[
                _section('Mening to\'plamlarim'),
                if (mine.isEmpty)
                  _hint(_svc.loading && !_svc.loaded
                      ? 'Yuklanmoqda...'
                      : 'Hali to\'plam yo\'q. "Yangi to\'plam" tugmasi bilan '
                          'o\'zingiznikini yarating: rasmlar admin ko\'rib '
                          'chiqqach to\'plamga qo\'shiladi.'),
                for (final p in mine) _tile(p, mine: true),
                const SizedBox(height: 14),
                _section('Qo\'shilgan to\'plamlar'),
                if (subs.isEmpty)
                  _hint('Boshqalarning to\'plamlarini pastdagi tugma orqali '
                      'qo\'shing.'),
                for (final p in subs) _tile(p, mine: false),
                const SizedBox(height: 12),
                GlassTappable(
                  onTap: () => Navigator.of(context).push(
                    MaterialPageRoute<void>(
                        builder: (_) => PackBrowseScreen(kind: _kind)),
                  ),
                  child: Glass(
                    borderRadius: 18,
                    padding: const EdgeInsets.all(16),
                    child: Row(
                      children: [
                        const Icon(Icons.explore_rounded,
                            color: AppColors.accent, size: 24),
                        const SizedBox(width: 14),
                        Expanded(
                          child: Text(
                            'Ommaviy ${PackKind.plural(_kind).toLowerCase()}ni ko\'rish',
                            style: const TextStyle(
                                color: Colors.white,
                                fontSize: 15,
                                fontWeight: FontWeight.w600),
                          ),
                        ),
                        const Icon(Icons.chevron_right_rounded,
                            color: Colors.white38),
                      ],
                    ),
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _kinds() {
    return Row(
      children: [
        for (final k in PackKind.all) ...[
          Expanded(
            child: GestureDetector(
              onTap: () => setState(() => _kind = k),
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 180),
                padding: const EdgeInsets.symmetric(vertical: 11),
                decoration: BoxDecoration(
                  color: _kind == k
                      ? AppColors.accent
                      : Colors.white.withValues(alpha: 0.07),
                  borderRadius: BorderRadius.circular(14),
                ),
                child: Center(
                  child: Text(
                    PackKind.plural(k),
                    style: TextStyle(
                      color: Colors.white
                          .withValues(alpha: _kind == k ? 1 : 0.7),
                      fontWeight: FontWeight.w600,
                      fontSize: 14,
                    ),
                  ),
                ),
              ),
            ),
          ),
          if (k != PackKind.all.last) const SizedBox(width: 8),
        ],
      ],
    );
  }

  Widget _section(String t) => Padding(
        padding: const EdgeInsets.only(left: 4, bottom: 8),
        child: Text(t,
            style: TextStyle(
                color: Colors.white.withValues(alpha: 0.6),
                fontSize: 13,
                fontWeight: FontWeight.w700)),
      );

  Widget _hint(String t) => Padding(
        padding: const EdgeInsets.fromLTRB(4, 0, 4, 10),
        child: Text(t,
            style: TextStyle(
                color: Colors.white.withValues(alpha: 0.5),
                fontSize: 13,
                height: 1.45)),
      );

  Widget _tile(PackInfo p, {required bool mine}) {
    final ops = mine ? _svc.opsOf(p.id) : const <PackOp>[];
    final waiting = ops.where((o) => o.isAdd && o.state != 'rejected').length;
    final rejected = ops.where((o) => o.isAdd && o.state == 'rejected').length;
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: GlassTappable(
        onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
            builder: (_) => PackDetailScreen(packId: p.id, initial: p))),
        child: Glass(
          borderRadius: 18,
          padding: const EdgeInsets.all(12),
          child: Row(
            children: [
              _cover(p),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(p.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                            color: Colors.white,
                            fontSize: 15.5,
                            fontWeight: FontWeight.w600)),
                    const SizedBox(height: 3),
                    Text(
                      '${p.items} ta · ${_size(p.bytes)}'
                      '${waiting > 0 ? ' · $waiting ta kutmoqda' : ''}'
                      '${rejected > 0 ? ' · $rejected ta rad etilgan' : ''}',
                      style: TextStyle(
                          color: rejected > 0
                              ? AppColors.gold
                              : Colors.white.withValues(alpha: 0.5),
                          fontSize: 12.5),
                    ),
                  ],
                ),
              ),
              const Icon(Icons.chevron_right_rounded, color: Colors.white38),
            ],
          ),
        ),
      ),
    );
  }

  Widget _cover(PackInfo p) => _PackCover(pack: p, size: 52);
}

String _size(int bytes) {
  if (bytes <= 0) return '0 KB';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).ceil()} KB';
  return '${(bytes / 1048576).toStringAsFixed(1)} MB';
}

/// To'plam muqovasi: birinchi elementning kichik rasmi.
class _PackCover extends StatefulWidget {
  final PackInfo pack;
  final double size;
  const _PackCover({required this.pack, required this.size});

  @override
  State<_PackCover> createState() => _PackCoverState();
}

class _PackCoverState extends State<_PackCover> {
  int _first = 0;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  @override
  void didUpdateWidget(_PackCover old) {
    super.didUpdateWidget(old);
    if (old.pack.version != widget.pack.version || old.pack.id != widget.pack.id) {
      unawaited(_load());
    }
  }

  Future<void> _load() async {
    if (!widget.pack.usable) return;
    final h = await PackService.instance.header(widget.pack);
    if (!mounted || h == null || h.items.isEmpty) return;
    setState(() => _first = h.items.first.id);
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      width: widget.size,
      height: widget.size,
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.06),
        borderRadius: BorderRadius.circular(14),
      ),
      child: _first == 0
          ? Icon(
              widget.pack.kind == PackKind.gif
                  ? Icons.gif_box_outlined
                  : Icons.emoji_emotions_outlined,
              color: Colors.white38)
          : Padding(
              padding: const EdgeInsets.all(6),
              child: PackImage(
                  pack: widget.pack.id,
                  item: _first,
                  size: widget.size - 12),
            ),
    );
  }
}

// ═══════════════════════════════════════════════════════════════
//  OMMAVIY TO'PLAMLAR
// ═══════════════════════════════════════════════════════════════

class PackBrowseScreen extends StatefulWidget {
  final String kind;
  const PackBrowseScreen({super.key, required this.kind});

  @override
  State<PackBrowseScreen> createState() => _PackBrowseScreenState();
}

class _PackBrowseScreenState extends State<PackBrowseScreen> {
  final _svc = PackService.instance;
  final List<PackInfo> _list = [];
  bool _loading = true;
  bool _more = true;
  bool _failed = false;
  final _scroll = ScrollController();

  @override
  void initState() {
    super.initState();
    _svc.addListener(_onSvc);
    _scroll.addListener(() {
      if (_scroll.position.pixels > _scroll.position.maxScrollExtent - 300) {
        unawaited(_next());
      }
    });
    unawaited(_next());
  }

  @override
  void dispose() {
    _svc.removeListener(_onSvc);
    _scroll.dispose();
    super.dispose();
  }

  void _onSvc() {
    if (mounted) setState(() {});
  }

  Future<void> _next() async {
    if (!_more || (_loading && _list.isNotEmpty)) return;
    setState(() {
      _loading = true;
      _failed = false;
    });
    final before = _list.isEmpty ? 0 : _svc.createdAtOf(_list.last.id);
    final r = await _svc.browse(kind: widget.kind, before: before);
    if (!mounted) return;
    setState(() {
      _loading = false;
      if (r == null) {
        _failed = true;
      } else {
        _list.addAll(r.where((p) => !_list.any((x) => x.id == p.id)));
        _more = r.length >= 30;
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final me = AuthService.instance.user?.id ?? 0;
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          title: Text('Ommaviy ${PackKind.plural(widget.kind).toLowerCase()}',
              style: const TextStyle(color: Colors.white, fontSize: 18)),
        ),
        body: _list.isEmpty
            ? Center(
                child: _loading
                    ? const CircularProgressIndicator(
                        strokeWidth: 2.4, color: Colors.white54)
                    : Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            _failed
                                ? 'Ro\'yxatni olib bo\'lmadi'
                                : 'Hali ommaviy to\'plam yo\'q',
                            style: const TextStyle(color: Colors.white60),
                          ),
                          if (_failed)
                            TextButton(
                                onPressed: _next,
                                child: const Text('Qayta urinish')),
                        ],
                      ),
              )
            : ListView.builder(
                controller: _scroll,
                padding: const EdgeInsets.fromLTRB(16, 4, 16, 40),
                itemCount: _list.length + (_more ? 1 : 0),
                itemBuilder: (_, i) {
                  if (i >= _list.length) {
                    return const Padding(
                      padding: EdgeInsets.all(16),
                      child: Center(
                          child: SizedBox(
                              width: 22,
                              height: 22,
                              child: CircularProgressIndicator(
                                  strokeWidth: 2, color: Colors.white54))),
                    );
                  }
                  final p = _list[i];
                  final on = _svc.isSubscribed(p.id) || p.ownerId == me;
                  return Padding(
                    padding: const EdgeInsets.only(bottom: 10),
                    child: GlassTappable(
                      onTap: () =>
                          Navigator.of(context).push(MaterialPageRoute<void>(
                        builder: (_) =>
                            PackDetailScreen(packId: p.id, initial: p),
                      )),
                      child: Glass(
                        borderRadius: 18,
                        padding: const EdgeInsets.all(12),
                        child: Row(
                          children: [
                            _PackCover(pack: p, size: 52),
                            const SizedBox(width: 14),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(p.title,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: const TextStyle(
                                          color: Colors.white,
                                          fontSize: 15.5,
                                          fontWeight: FontWeight.w600)),
                                  const SizedBox(height: 3),
                                  Text(
                                    '${p.items} ta · ${_size(p.bytes)}'
                                    '${p.ownerName.isEmpty ? '' : ' · ${p.ownerName}'}',
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                        color: Colors.white
                                            .withValues(alpha: 0.5),
                                        fontSize: 12.5),
                                  ),
                                ],
                              ),
                            ),
                            const SizedBox(width: 8),
                            if (p.ownerId != me)
                              _SubButton(
                                on: on,
                                onTap: () => _svc.setSubscribed(p, !on),
                              ),
                          ],
                        ),
                      ),
                    ),
                  );
                },
              ),
      ),
    );
  }
}

/// "Qo'shish" / "Qo'shilgan" tugmasi.
class _SubButton extends StatelessWidget {
  final bool on;
  final VoidCallback onTap;
  const _SubButton({required this.on, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        decoration: BoxDecoration(
          color: on ? Colors.white.withValues(alpha: 0.08) : AppColors.accent,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Text(
          on ? 'Qo\'shilgan' : 'Qo\'shish',
          style: TextStyle(
              color: Colors.white.withValues(alpha: on ? 0.7 : 1),
              fontSize: 13,
              fontWeight: FontWeight.w600),
        ),
      ),
    );
  }
}
