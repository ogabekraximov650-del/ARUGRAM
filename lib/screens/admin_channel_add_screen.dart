// lib/screens/admin_channel_add_screen.dart — MAJBURIY KANAL QO'SHISH.
//
// TALAB (foydalanuvchi): "admin paneli orqali majburiy kanal qo'shganda
// yangi oyna ochilsin va yuqorida yozadigan joy bo'lsin. O'sha joyga kanal
// IDsi yoki useri qo'yilsa avval ilova kanal bor-yo'qligini tekshirsin va
// mavjud bo'lsa kanal haqidagi ma'lumotlarni chiqarsin: surati, nomi,
// username, obunachilar va hokazo." Limit esa ANIQ son bilan ("1000 desam
// 1000") yoki cheksiz.
//
// Tekshiruv adminning O'Z Telegram hisobi bilan (`rust_tg_channel_info`):
// @username, -100... ID yoki t.me/+havola. Qo'shishda avvalgidek asosiy
// bot kanalga admin qilinadi (`rust_tg_make_bot_admin`), keyin server
// o'zi yana tekshiradi (`POST /api/admin/channels`, `op: add`).
//
// Ekran `true` qaytaradi — kanal qo'shildi.

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:http/http.dart' as http;

import '../services/auth_service.dart';
import '../services/format.dart';
import '../services/telegram_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';

class AdminChannelAddScreen extends StatefulWidget {
  const AdminChannelAddScreen({super.key});

  @override
  State<AdminChannelAddScreen> createState() => _AdminChannelAddScreenState();
}

class _AdminChannelAddScreenState extends State<AdminChannelAddScreen> {
  final _input = TextEditingController();
  final _need = TextEditingController();
  bool _unlimited = true;

  /// `public` | `private` — topilgan kanalga qarab o'zi tanlanadi,
  /// admin o'zgartira oladi.
  String _kind = 'public';

  Map<String, dynamic>? _info;
  Uint8List? _photo;
  String? _error;
  bool _checking = false;
  bool _adding = false;

  @override
  void dispose() {
    _input.dispose();
    _need.dispose();
    super.dispose();
  }

  Future<void> _check() async {
    final q = _input.text.trim();
    if (q.isEmpty || _checking) return;
    FocusScope.of(context).unfocus();
    if (!TelegramService.instance.authorized) {
      setState(() {
        _info = null;
        _error = 'Telegram hisobingiz ulanmagan — kanalni tekshirib bo\'lmaydi. '
            'Profil orqali Telegram\'ga ulaning.';
      });
      return;
    }
    setState(() {
      _checking = true;
      _error = null;
      _info = null;
      _photo = null;
    });
    final j = await tgCall('rust_tg_channel_info', arg: q);
    if (!mounted) return;
    if (j['ok'] != true) {
      setState(() {
        _checking = false;
        _error = '${j['error'] ?? 'Kanal topilmadi'}';
      });
      return;
    }
    Uint8List? photo;
    final p = '${j['photo'] ?? ''}';
    if (p.isNotEmpty) {
      try {
        photo = base64Decode(p);
      } catch (_) {}
    }
    setState(() {
      _checking = false;
      _info = j;
      _photo = photo;
      _kind = '${j['username'] ?? ''}'.isNotEmpty ? 'public' : 'private';
    });
  }

  Future<void> _add() async {
    final info = _info;
    if (info == null || _adding) return;
    final need = _unlimited ? 0 : (int.tryParse(_need.text) ?? 0);
    if (!_unlimited && need <= 0) {
      setState(() => _error = 'Limitni yozing (masalan 1000) yoki "Cheksiz"ni yoqing');
      return;
    }
    setState(() {
      _adding = true;
      _error = null;
    });
    // Kanal IDsi ma'lum bo'lsa — o'sha; bo'lmasa (a'zo bo'lmagan yopiq
    // kanal havolasi) — yozilgan matn.
    final id = (info['id'] as num?)?.toInt() ?? 0;
    var target = id != 0 ? '$id' : _input.text.trim();
    String? adminErr;
    final j = await tgCall('rust_tg_make_bot_admin', arg: target);
    if (!mounted) return;
    final cid = (j['chat_id'] as num?)?.toInt();
    if (j['ok'] == true && cid != null) {
      target = '$cid';
    } else {
      adminErr = '${j['error'] ?? ''}';
    }
    String? err;
    try {
      final r = await http
          .post(
            Uri.parse('$kApiBase/api/admin/channels'),
            headers: {
              'Authorization': 'Bearer ${AuthService.instance.sessionToken}',
              'Content-Type': 'application/json',
            },
            body: jsonEncode(
                {'op': 'add', 'kind': _kind, 'input': target, 'need': need}),
          )
          .timeout(const Duration(seconds: 30));
      if (r.statusCode != 200) {
        try {
          err = '${(jsonDecode(r.body) as Map)['error']}';
        } catch (_) {
          err = 'Xato (${r.statusCode})';
        }
      }
    } catch (_) {
      err = 'Internet yo\'q';
    }
    if (!mounted) return;
    if (err == null) {
      Navigator.of(context).pop(true);
      return;
    }
    setState(() {
      _adding = false;
      _error = adminErr != null && adminErr.isNotEmpty
          ? '$err\n(Botni admin qilib bo\'lmadi: $adminErr)'
          : err;
    });
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
          title: const Text('Kanal qo\'shish',
              style: TextStyle(color: Colors.white, fontSize: 18)),
        ),
        body: SafeArea(
          top: false,
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 4, 16, 32),
            children: [
              // ── QIDIRUV ─────────────────────────────────────
              Glass(
                borderRadius: 16,
                padding: const EdgeInsets.fromLTRB(14, 4, 6, 4),
                child: Row(
                  children: [
                    const Icon(Icons.search_rounded, color: Colors.white54),
                    const SizedBox(width: 8),
                    Expanded(
                      child: TextField(
                        controller: _input,
                        autofocus: true,
                        textInputAction: TextInputAction.search,
                        onSubmitted: (_) => _check(),
                        style: const TextStyle(color: Colors.white),
                        decoration: const InputDecoration(
                          border: InputBorder.none,
                          hintText: '@username, -100... ID yoki t.me/+havola',
                          hintStyle: TextStyle(color: Colors.white38),
                        ),
                      ),
                    ),
                    _checking
                        ? const Padding(
                            padding: EdgeInsets.all(12),
                            child: SizedBox(
                              width: 20,
                              height: 20,
                              child: CircularProgressIndicator(
                                  strokeWidth: 2, color: Colors.white70),
                            ),
                          )
                        : TextButton(
                            onPressed: _check,
                            child: const Text('Tekshirish'),
                          ),
                  ],
                ),
              ),
              const SizedBox(height: 8),
              Text(
                'Ilova kanalni Telegram hisobingiz orqali tekshiradi. Qo\'shishda '
                'asosiy bot kanalga admin qilinadi — siz kanal egasi (yoki admin '
                'qo\'sha oladigan admini) bo\'lishingiz kerak.',
                style: TextStyle(
                    color: Colors.white.withValues(alpha: 0.4), fontSize: 12),
              ),
              if (_error != null) ...[
                const SizedBox(height: 14),
                Glass(
                  borderRadius: 14,
                  padding: const EdgeInsets.all(12),
                  child: Row(
                    children: [
                      const Icon(Icons.error_outline_rounded,
                          color: AppColors.danger),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Text(_error!,
                            style: const TextStyle(
                                color: Colors.white, fontSize: 13)),
                      ),
                    ],
                  ),
                ),
              ],
              if (_info != null) ...[
                const SizedBox(height: 16),
                _InfoCard(info: _info!, photo: _photo),
                const SizedBox(height: 16),
                _section('KANAL TURI'),
                const SizedBox(height: 8),
                Row(
                  children: [
                    _choice('public', Icons.campaign_rounded, 'Ochiq',
                        'Ilova o\'zi qo\'shadi'),
                    const SizedBox(width: 10),
                    _choice('private', Icons.lock_rounded, 'Yopiq',
                        'So\'rov yuboradi'),
                  ],
                ),
                const SizedBox(height: 16),
                _section('LIMIT — NECHTA ODAM'),
                const SizedBox(height: 8),
                Glass(
                  borderRadius: 16,
                  padding: const EdgeInsets.fromLTRB(14, 6, 10, 6),
                  child: Column(
                    children: [
                      Row(
                        children: [
                          const Expanded(
                            child: Text('Cheksiz',
                                style: TextStyle(
                                    color: Colors.white, fontSize: 14.5)),
                          ),
                          Switch(
                            value: _unlimited,
                            activeThumbColor: Colors.white,
                            activeTrackColor: AppColors.accent,
                            onChanged: (v) => setState(() => _unlimited = v),
                          ),
                        ],
                      ),
                      if (!_unlimited)
                        TextField(
                          controller: _need,
                          keyboardType: TextInputType.number,
                          inputFormatters: [
                            FilteringTextInputFormatter.digitsOnly
                          ],
                          style: const TextStyle(
                              color: Colors.white, fontSize: 18),
                          decoration: const InputDecoration(
                            hintText: 'Masalan: 1000',
                            hintStyle: TextStyle(color: Colors.white38),
                          ),
                        ),
                    ],
                  ),
                ),
                const SizedBox(height: 22),
                SizedBox(
                  height: 50,
                  child: FilledButton.icon(
                    style: FilledButton.styleFrom(
                      backgroundColor: AppColors.accent,
                      shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(14)),
                    ),
                    onPressed: _adding ? null : _add,
                    icon: _adding
                        ? const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(
                                strokeWidth: 2, color: Colors.white),
                          )
                        : const Icon(Icons.add_rounded),
                    label: const Text('Qo\'shish',
                        style: TextStyle(
                            fontSize: 15.5, fontWeight: FontWeight.w700)),
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _section(String t) => Padding(
        padding: const EdgeInsets.only(left: 4),
        child: Text(t,
            style: TextStyle(
                color: Colors.white.withValues(alpha: 0.4),
                fontSize: 11,
                fontWeight: FontWeight.w700,
                letterSpacing: 0.4)),
      );

  Widget _choice(String k, IconData icon, String title, String sub) {
    final on = _kind == k;
    return Expanded(
      child: GestureDetector(
        onTap: () => setState(() => _kind = k),
        child: Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: on
                ? AppColors.accent.withValues(alpha: 0.16)
                : Colors.white.withValues(alpha: 0.05),
            borderRadius: BorderRadius.circular(14),
            border: Border.all(
                color: on ? AppColors.accent : Colors.white12, width: 1.2),
          ),
          child: Row(
            children: [
              Icon(icon, color: on ? AppColors.accent2 : Colors.white54),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title,
                        style: const TextStyle(
                            color: Colors.white,
                            fontWeight: FontWeight.w700,
                            fontSize: 14)),
                    Text(sub,
                        style: const TextStyle(
                            color: Colors.white54, fontSize: 11.5)),
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

/// Topilgan kanal: surat, nom, username, obunachilar, tavsif, belgilar.
class _InfoCard extends StatelessWidget {
  final Map<String, dynamic> info;
  final Uint8List? photo;
  const _InfoCard({required this.info, this.photo});

  @override
  Widget build(BuildContext context) {
    final title = '${info['title'] ?? ''}';
    final username = '${info['username'] ?? ''}';
    final about = '${info['about'] ?? ''}'.trim();
    final members = (info['members'] as num?)?.toInt() ?? 0;
    final id = (info['id'] as num?)?.toInt() ?? 0;
    final broadcast = info['broadcast'] == true;
    final p = photo;
    return Glass(
      borderRadius: 18,
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              ClipOval(
                child: SizedBox(
                  width: 64,
                  height: 64,
                  child: p != null
                      ? Image.memory(p, fit: BoxFit.cover)
                      : Container(
                          color: AppColors.accent.withValues(alpha: 0.25),
                          alignment: Alignment.center,
                          child: Text(
                            title.isNotEmpty ? title[0].toUpperCase() : '?',
                            style: const TextStyle(
                                color: Colors.white,
                                fontSize: 26,
                                fontWeight: FontWeight.w800),
                          ),
                        ),
                ),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Flexible(
                          child: Text(title,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                  color: Colors.white,
                                  fontSize: 16.5,
                                  fontWeight: FontWeight.w800)),
                        ),
                        if (info['verified'] == true) ...[
                          const SizedBox(width: 4),
                          const Icon(Icons.verified_rounded,
                              color: AppColors.telegramLight, size: 18),
                        ],
                      ],
                    ),
                    if (username.isNotEmpty)
                      Text('@$username',
                          style: const TextStyle(
                              color: AppColors.telegramLight, fontSize: 13.5)),
                    const SizedBox(height: 2),
                    Text(
                      '${formatCount(members)} ${broadcast ? 'obunachi' : 'a\'zo'}',
                      style:
                          const TextStyle(color: Colors.white70, fontSize: 13),
                    ),
                  ],
                ),
              ),
            ],
          ),
          if (about.isNotEmpty) ...[
            const SizedBox(height: 12),
            Text(about,
                maxLines: 6,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                    color: Colors.white70, fontSize: 13, height: 1.35)),
          ],
          const SizedBox(height: 12),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: [
              _tag(broadcast ? 'Kanal' : 'Guruh', Icons.campaign_outlined),
              _tag(username.isNotEmpty ? 'Ochiq' : 'Yopiq',
                  username.isNotEmpty ? Icons.public : Icons.lock_outline),
              if (info['join_request'] == true)
                _tag('So\'rov bilan qo\'shiladi', Icons.how_to_reg_outlined),
              _tag(info['member'] == true ? 'Siz a\'zosiz' : 'Siz a\'zo emassiz',
                  Icons.person_outline),
              if (id != 0) _tag('ID: $id', Icons.tag),
            ],
          ),
        ],
      ),
    );
  }

  static Widget _tag(String t, IconData icon) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
        decoration: BoxDecoration(
          color: Colors.white.withValues(alpha: 0.06),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 13, color: Colors.white60),
            const SizedBox(width: 4),
            Text(t,
                style: const TextStyle(color: Colors.white70, fontSize: 11.5)),
          ],
        ),
      );
}
