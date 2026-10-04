// lib/screens/admin_channels_screen.dart — MAJBURIY OBUNA KANALLARI.
//
// TALAB (foydalanuvchi): admin panelida to'liq tizim — kanal qo'shish
// (ochiq / yopiq), limit, statistika va o'chirish.
// Xuddi shu amallar ASOSIY botda ham bor ("🔐 Majburiy obunalar").
// Ikkalasi ham bitta server kodini ishlatadi (`worker/src/channels.rs`).
//
// Asosiy bot kanalda ADMIN bo'lishi shart (yopiq kanal havolasini bot
// yaratadi, kim qo'shilgani/so'rov yuborganini bot sanaydi). Bu ekranda
// admin faqat @username yoki ID yozadi — ilova adminning O'Z Telegram
// hisobi bilan kanalni topib, botni o'zi admin qiladi
// (`rust_tg_make_bot_admin`), ya'ni Telegram botiga kirish shart emas.

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:http/http.dart' as http;

import '../services/auth_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../widgets/trend_chart.dart';
import '../services/format.dart';
import 'admin_channel_add_screen.dart';

class AdminChannelsScreen extends StatefulWidget {
  const AdminChannelsScreen({super.key});

  @override
  State<AdminChannelsScreen> createState() => _AdminChannelsScreenState();
}

class _AdminChannelsScreenState extends State<AdminChannelsScreen> {
  List<Map<String, dynamic>> _items = const [];
  bool _loading = true;
  bool _busy = false;
  String? _error;

  Map<String, String> get _headers => {
        'Authorization': 'Bearer ${AuthService.instance.sessionToken}',
        'Content-Type': 'application/json',
      };

  @override
  void initState() {
    super.initState();
    _load();
  }

  void _say(String text) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(
      behavior: SnackBarBehavior.floating,
      backgroundColor: AppColors.card,
      content: Text(text, style: const TextStyle(color: Colors.white)),
    ));
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final r = await http
          .get(Uri.parse('$kApiBase/api/admin/channels'), headers: _headers)
          .timeout(const Duration(seconds: 20));
      if (!mounted) return;
      if (r.statusCode == 200) {
        final j = jsonDecode(r.body) as Map<String, dynamic>;
        setState(() {
          _items = (j['items'] as List? ?? const [])
              .whereType<Map<String, dynamic>>()
              .toList();
          _loading = false;
        });
      } else {
        setState(() {
          _error = 'Yuklanmadi (${r.statusCode})';
          _loading = false;
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() {
          _error = 'Internet yo\'q';
          _loading = false;
        });
      }
    }
  }

  /// `POST /api/admin/channels`. Xato bo'lsa matni qaytadi.
  Future<String?> _post(Map<String, dynamic> body) async {
    setState(() => _busy = true);
    try {
      final r = await http
          .post(Uri.parse('$kApiBase/api/admin/channels'),
              headers: _headers, body: jsonEncode(body))
          .timeout(const Duration(seconds: 30));
      if (r.statusCode == 200) {
        await _load();
        return null;
      }
      try {
        return '${(jsonDecode(r.body) as Map)['error']}';
      } catch (_) {
        return 'Xato (${r.statusCode})';
      }
    } catch (_) {
      return 'Internet yo\'q';
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  // ── QO'SHISH ─────────────────────────────────────────────

  /// Yangi oyna: avval kanal tekshiriladi (surati, nomi, obunachilar),
  /// keyin limit va qo'shish (`admin_channel_add_screen.dart`).
  Future<void> _add() async {
    final added = await Navigator.of(context).push<bool>(
      MaterialPageRoute(builder: (_) => const AdminChannelAddScreen()),
    );
    if (added == true && mounted) {
      _say('Qo\'shildi');
      await _load();
    }
  }

  // ── LIMIT VA O'CHIRISH ───────────────────────────────────

  /// Limit ANIQ son bilan (foydalanuvchi talabi: "1000 desam 1000,
  /// 10 000 desam 10 000"). "Cheksiz" — 0.
  Future<void> _limit(Map<String, dynamic> c) async {
    final cur = (c['need'] as num?)?.toInt() ?? 0;
    final ctl = TextEditingController(text: cur > 0 ? '$cur' : '');
    final n = await showDialog<int>(
      context: context,
      builder: (d) => AlertDialog(
        backgroundColor: AppColors.card,
        title: const Text('Limitni belgilash',
            style: TextStyle(color: Colors.white)),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Hozir: ${cur > 0 ? formatCount(cur) : 'cheksiz'}',
                style: const TextStyle(color: Colors.white60, fontSize: 13)),
            TextField(
              controller: ctl,
              autofocus: true,
              keyboardType: TextInputType.number,
              inputFormatters: [FilteringTextInputFormatter.digitsOnly],
              style: const TextStyle(color: Colors.white, fontSize: 18),
              decoration: const InputDecoration(
                  labelText: 'Nechta odam (masalan 1000)'),
            ),
          ],
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(d), child: const Text('Bekor')),
          TextButton(
              onPressed: () => Navigator.pop(d, 0),
              child: const Text('Cheksiz')),
          FilledButton(
              onPressed: () {
                final v = int.tryParse(ctl.text) ?? 0;
                if (v > 0) Navigator.pop(d, v);
              },
              child: const Text('Saqlash')),
        ],
      ),
    );
    if (n == null || n < 0) return;
    final err = await _post({'op': 'set', 'id': c['id'], 'need': n});
    _say(err ?? 'Saqlandi');
  }

  /// Kanal grafigi (qo'shilganlar / so'rov yuborganlar, kunlar bo'yicha).
  void _chart(Map<String, dynamic> c) {
    final kind = '${c['kind']}';
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: AppColors.card,
      isScrollControlled: true,
      shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.vertical(top: Radius.circular(20))),
      builder: (_) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('${c['title']}',
                  style: const TextStyle(
                      color: Colors.white,
                      fontSize: 16,
                      fontWeight: FontWeight.w700)),
              Text(
                  kind == 'public'
                      ? 'Kanalga qo\'shilganlar'
                      : 'Qo\'shilish so\'rovini yuborganlar',
                  style: const TextStyle(color: Colors.white54, fontSize: 12.5)),
              const SizedBox(height: 14),
              TrendChart(
                metric: 'chan:${c['id']}',
                format: (v) => '${formatCount(v)} ta',
                height: 170,
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _delete(Map<String, dynamic> c) async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (d) => AlertDialog(
        backgroundColor: AppColors.card,
        title: const Text('O\'chirilsinmi?',
            style: TextStyle(color: Colors.white)),
        content: Text('${c['title']}',
            style: const TextStyle(color: Colors.white70)),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(d, false),
              child: const Text('Bekor')),
          FilledButton(
              style: FilledButton.styleFrom(backgroundColor: AppColors.danger),
              onPressed: () => Navigator.pop(d, true),
              child: const Text('O\'chirish')),
        ],
      ),
    );
    if (ok != true) return;
    final err = await _post({'op': 'del', 'id': c['id']});
    _say(err ?? 'O\'chirildi');
  }

  // ── EKRAN ────────────────────────────────────────────────

  Widget _card(Map<String, dynamic> c) {
    final kind = '${c['kind']}';
    final need = (c['need'] as num?)?.toInt() ?? 0;
    final joined = (c['joined'] as num?)?.toInt() ?? 0;
    final verb = kind == 'public' ? 'qo\'shilgan' : 'so\'rov yuborgan';
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Glass(
        borderRadius: 18,
        blur: 14,
        padding: const EdgeInsets.fromLTRB(16, 12, 8, 8),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            Icon(kind == 'public' ? Icons.campaign_rounded : Icons.lock_rounded,
                color: Colors.white70, size: 20),
            const SizedBox(width: 8),
            Expanded(
              child: Text('${c['title']}',
                  style: const TextStyle(
                      color: Colors.white,
                      fontSize: 16,
                      fontWeight: FontWeight.w700)),
            ),
            if (c['active'] == false)
              const Text('limit to\'ldi',
                  style: TextStyle(color: AppColors.accent, fontSize: 12)),
          ]),
          const SizedBox(height: 4),
          SelectableText('${c['url']}',
              style: const TextStyle(color: Colors.white54, fontSize: 12.5)),
          ...[
            const SizedBox(height: 4),
            Text('$joined / ${need > 0 ? need : '∞'} tasi $verb',
                style: const TextStyle(color: Colors.white70, fontSize: 13)),
          ],
          Row(mainAxisAlignment: MainAxisAlignment.end, children: [
            IconButton(
              tooltip: 'Statistika',
              onPressed: () => _chart(c),
              icon: const Icon(Icons.show_chart_rounded,
                  color: Colors.white70),
            ),
            IconButton(
              tooltip: 'Limitni belgilash',
              onPressed: _busy ? null : () => _limit(c),
              icon: const Icon(Icons.edit_rounded, color: Colors.white70),
            ),
            IconButton(
              tooltip: 'O\'chirish',
              onPressed: _busy ? null : () => _delete(c),
              icon: const Icon(Icons.delete_outline_rounded,
                  color: AppColors.danger),
            ),
          ]),
        ]),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          title: const Text('Majburiy obunalar',
              style: TextStyle(color: Colors.white)),
        ),
        floatingActionButton: FloatingActionButton.extended(
          backgroundColor: AppColors.accent,
          onPressed: _busy || _loading ? null : _add,
          icon: const Icon(Icons.add_rounded),
          label: const Text('Qo\'shish'),
        ),
        body: _loading
            ? const Center(child: CircularProgressIndicator())
            : _error != null
                ? Center(
                    child: TextButton(
                        onPressed: _load,
                        child: Text('$_error — qayta urinish')))
                : RefreshIndicator(
                    onRefresh: _load,
                    child: ListView(
                      padding: const EdgeInsets.fromLTRB(16, 8, 16, 96),
                      children: [
                        Text(
                          'Kanallar: ${_items.length}. Bepul bo\'limni ochishdan oldin ilova '
                          'ruxsat so\'raydi va foydalanuvchining Telegram hisobi bilan shu '
                          'kanallarga o\'zi qo\'shiladi (yopiq kanalga so\'rov yuboradi). '
                          'Limit to\'lgan kanal endi talab qilinmaydi.',
                          style: const TextStyle(
                              color: Colors.white60,
                              fontSize: 12.5,
                              height: 1.45),
                        ),
                        const SizedBox(height: 12),
                        // Hamma kanal bo'yicha — treyding chizig'idek.
                        if (_items.isNotEmpty) ...[
                          Glass(
                            borderRadius: 18,
                            padding: const EdgeInsets.all(14),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                const Text('Barcha kanallar',
                                    style: TextStyle(
                                        color: Colors.white,
                                        fontWeight: FontWeight.w700,
                                        fontSize: 15)),
                                const Text(
                                    'Qo\'shilganlar va so\'rov yuborganlar',
                                    style: TextStyle(
                                        color: Colors.white54, fontSize: 12)),
                                const SizedBox(height: 12),
                                TrendChart(
                                  metric: 'chan',
                                  format: (v) => '${formatCount(v)} ta',
                                ),
                              ],
                            ),
                          ),
                          const SizedBox(height: 12),
                        ],
                        if (_items.isEmpty)
                          const Padding(
                            padding: EdgeInsets.only(top: 40),
                            child: Center(
                              child: Text('Hali kanal qo\'shilmagan',
                                  style: TextStyle(color: Colors.white54)),
                            ),
                          ),
                        for (final c in _items) _card(c),
                      ],
                    ),
                  ),
      ),
    );
  }
}
