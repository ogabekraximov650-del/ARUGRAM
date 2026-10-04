// lib/screens/admin_encode_screen.dart — KODLASH NAVBATI (admin).
//
// TALAB (foydalanuvchi): "admin paneliga kodlash uchun navbatda turgan va
// encode qilinayotgan animelar bo'limini qo'sh. Yuqorida hozir
// kodlanayotgan: bo'lim nomi, nechanchi bo'lim va nechanchi qismligi va
// encode log statistikasi. Pastida navbatda turgan: bo'lim surati, bo'lim
// nomi, nechanchi bo'limligi va nechanchi qismligi."
//
// ── MA'LUMOT QAYERDAN ───────────────────────────────────────
//
//   * `GET /api/encode/admin` — navbat (bazadan bitta O'QISH);
//   * `GET /api/encode/live` — jonli holat. Worker uni FAQAT shu so'rovda
//     oladi: log kanalidagi qadalgan `#arustatus` xabari (runner ~15
//     soniyada yangilaydi) va GitHub'dagi run qadamlari. Bazaga hech
//     narsa yozilmaydi (foydalanuvchi talabi: "har 10 daqiqada bazaga log
//     yozish shart bo'lmasin").
//
// Ekran o'zi yangilanmaydi — yuqoridagi "Yangilash" tugmasi bosilganda.

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

import '../services/auth_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../widgets/poster_image.dart';

class AdminEncodeScreen extends StatefulWidget {
  const AdminEncodeScreen({super.key});

  @override
  State<AdminEncodeScreen> createState() => _AdminEncodeScreenState();
}

class _AdminEncodeScreenState extends State<AdminEncodeScreen> {
  List<Map<String, dynamic>> _running = const [];
  List<Map<String, dynamic>> _queue = const [];
  List<Map<String, dynamic>> _errors = const [];
  Map<String, dynamic> _live = const {};
  bool _loading = true;
  String? _error;
  DateTime? _at;

  Map<String, String> get _headers => {
        'Authorization': 'Bearer ${AuthService.instance.sessionToken}',
      };

  @override
  void initState() {
    super.initState();
    _load();
  }

  static List<Map<String, dynamic>> _list(Object? v) =>
      (v as List? ?? const []).whereType<Map<String, dynamic>>().toList();

  Future<Map<String, dynamic>?> _get(String path) async {
    final r = await http
        .get(Uri.parse('$kApiBase$path'), headers: _headers)
        .timeout(const Duration(seconds: 25));
    if (r.statusCode != 200) {
      throw 'Yuklanmadi (${r.statusCode})';
    }
    return jsonDecode(r.body) as Map<String, dynamic>;
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final res = await Future.wait([
        _get('/api/encode/admin'),
        _get('/api/encode/live'),
      ]);
      if (!mounted) return;
      final q = res[0] ?? const {};
      setState(() {
        _running = _list(q['running']);
        _queue = _list(q['queue']);
        _errors = _list(q['errors']);
        _live = res[1] ?? const {};
        _loading = false;
        _at = DateTime.now();
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e is String ? e : 'Internet yo\'q';
        _loading = false;
      });
    }
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
          title: const Text('Kodlash navbati',
              style: TextStyle(color: Colors.white, fontSize: 18)),
          actions: [
            IconButton(
              tooltip: 'Yangilash',
              onPressed: _loading ? null : _load,
              icon: _loading
                  ? const SizedBox(
                      width: 20,
                      height: 20,
                      child: CircularProgressIndicator(
                          strokeWidth: 2, color: Colors.white70),
                    )
                  : const Icon(Icons.refresh_rounded),
            ),
          ],
        ),
        body: SafeArea(
          top: false,
          child: RefreshIndicator(
            onRefresh: _load,
            color: AppColors.accent,
            child: ListView(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 32),
              physics: const AlwaysScrollableScrollPhysics(),
              children: _children(),
            ),
          ),
        ),
      ),
    );
  }

  List<Widget> _children() {
    if (_loading && _at == null) {
      return const [
        SizedBox(height: 120),
        Center(child: CircularProgressIndicator(color: AppColors.accent)),
      ];
    }
    if (_error != null && _at == null) {
      return [
        const SizedBox(height: 100),
        Center(
          child: Text(_error!,
              style: const TextStyle(color: Colors.white70, fontSize: 14)),
        ),
      ];
    }
    final status = _live['status'] is Map<String, dynamic>
        ? _live['status'] as Map<String, dynamic>
        : null;
    final github = _live['github'] is Map<String, dynamic>
        ? _live['github'] as Map<String, dynamic>
        : null;
    return [
      if (_error != null) ...[
        Text(_error!, style: const TextStyle(color: AppColors.danger)),
        const SizedBox(height: 8),
      ],
      const _Label('HOZIR KODLANMOQDA'),
      const SizedBox(height: 8),
      if (_running.isEmpty)
        const _Empty('Hozir hech narsa kodlanmayapti')
      else
        for (final j in _running) ...[
          _RunningCard(job: j, status: _matches(status, j) ? status : null),
          const SizedBox(height: 10),
        ],
      const SizedBox(height: 6),
      _LiveCard(
        status: status,
        statusError: '${_live['status_error'] ?? ''}',
        github: github,
        fetchedAt: _at,
      ),
      const SizedBox(height: 22),
      _Label('NAVBATDA (${_queue.length})'),
      const SizedBox(height: 8),
      if (_queue.isEmpty)
        const _Empty('Navbat bo\'sh')
      else
        for (final j in _queue) ...[
          _QueueTile(job: j),
          const SizedBox(height: 8),
        ],
      if (_errors.isNotEmpty) ...[
        const SizedBox(height: 22),
        _Label('XATO BILAN TO\'XTAGAN (${_errors.length})'),
        const SizedBox(height: 8),
        for (final j in _errors) ...[
          _QueueTile(job: j, error: true),
          const SizedBox(height: 8),
        ],
      ],
    ];
  }

  /// Qadalgan holat shu ishga tegishlimi (runner boshqa qismga o'tgan
  /// bo'lishi mumkin).
  static bool _matches(Map<String, dynamic>? st, Map<String, dynamic> j) =>
      st != null &&
      '${st['anime_id']}' == '${j['anime_id']}' &&
      '${st['season_id']}' == '${j['season_id']}' &&
      '${st['epizod_id']}' == '${j['epizod_id']}';
}

// ── YORDAMCHILAR ──────────────────────────────────────────────

int _int(Object? v) => int.tryParse('${v ?? 0}') ?? 0;

/// "2-bo'lim · 5-qism".
String _where(Map<String, dynamic> j) {
  final b = _int(j['bolim_id']);
  final q = _int(j['epizod_number']);
  return [
    if (b > 0) '$b-bo\'lim',
    if (q > 0) '$q-qism',
  ].join(' · ');
}

String _title(Map<String, dynamic> j) {
  final n = '${j['nomi'] ?? ''}'.trim();
  final a = '${j['anime_name'] ?? ''}'.trim();
  if (n.isNotEmpty) return n;
  if (a.isNotEmpty) return a;
  return 'Anime #${j['anime_id']}';
}

String _clock(int s) {
  if (s < 0) return '--:--';
  final h = s ~/ 3600, m = (s % 3600) ~/ 60, sec = s % 60;
  String two(int v) => v.toString().padLeft(2, '0');
  return h > 0 ? '$h:${two(m)}:${two(sec)}' : '${two(m)}:${two(sec)}';
}

String _ago(int ms) {
  if (ms <= 0) return '';
  final s = (DateTime.now().millisecondsSinceEpoch - ms) ~/ 1000;
  if (s < 60) return '${s < 0 ? 0 : s} soniya oldin';
  if (s < 3600) return '${s ~/ 60} daqiqa oldin';
  return '${s ~/ 3600} soat oldin';
}

/// Runner jarayon satri (`enc|1080p|37|1|4|...`, `download`,
/// `upload|720p|2|4`) — ekran uchun.
class _Progress {
  final String stage;
  final String quality;
  final double? pct;
  final String step;
  final List<(String, String)> stats;
  const _Progress(this.stage, this.quality, this.pct, this.step, this.stats);

  static _Progress parse(String p) {
    final v = p.split('|');
    switch (v.first) {
      case 'download':
        return const _Progress('Asl video yuklab olinmoqda', '', null, '', []);
      case 'upload' when v.length >= 4:
        return _Progress('Telegram\'ga yuklanmoqda', v[1], null,
            '${v[2]}/${v[3]}', const []);
      case 'enc' when v.length >= 5:
        final stats = <(String, String)>[];
        if (v.length >= 12) {
          stats.addAll([
            ('Tezlik', v[5]),
            ('Kadr/s', v[6]),
            ('Bitreyt', '${v[7]} kb/s'),
            ('Hajm', '${v[8]} MB${v[9] == '-' ? '' : ' (~${v[9]} MB)'}'),
            ('O\'tdi', _clock(int.tryParse(v[10]) ?? -1)),
            ('Qoldi', '~${_clock(int.tryParse(v[11]) ?? -1)}'),
          ]);
        }
        return _Progress('Kodlanmoqda', v[1],
            (double.tryParse(v[2]) ?? 0) / 100, '${v[3]}/${v[4]}', stats);
      case 'start':
        return const _Progress('Boshlanmoqda', '', null, '', []);
      default:
        return const _Progress('Kutmoqda', '', null, '', []);
    }
  }
}

// ── VIDJETLAR ─────────────────────────────────────────────────

class _Label extends StatelessWidget {
  final String text;
  const _Label(this.text);

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(left: 4),
        child: Text(
          text,
          style: TextStyle(
            color: Colors.white.withValues(alpha: 0.4),
            fontSize: 11,
            fontWeight: FontWeight.w700,
            letterSpacing: 0.4,
          ),
        ),
      );
}

class _Empty extends StatelessWidget {
  final String text;
  const _Empty(this.text);

  @override
  Widget build(BuildContext context) => Glass(
        borderRadius: 16,
        padding: const EdgeInsets.all(16),
        child: Text(text,
            style: const TextStyle(color: Colors.white54, fontSize: 13.5)),
      );
}

class _Poster extends StatelessWidget {
  final String url;
  final double w;
  const _Poster(this.url, {this.w = 58});

  @override
  Widget build(BuildContext context) => ClipRRect(
        borderRadius: BorderRadius.circular(10),
        child: SizedBox(
          width: w,
          height: w * 1.4,
          child: url.isEmpty
              ? Container(
                  color: AppColors.cardAlt,
                  child: const Icon(Icons.movie_creation_outlined,
                      color: Colors.white38),
                )
              : PosterImage(url: url),
        ),
      );
}

/// Hozir kodlanayotgan qism: surat, nom, bo'lim/qism va jarayon.
class _RunningCard extends StatelessWidget {
  final Map<String, dynamic> job;
  final Map<String, dynamic>? status;
  const _RunningCard({required this.job, this.status});

  @override
  Widget build(BuildContext context) {
    final p = _Progress.parse('${status?['progress'] ?? ''}');
    final done = (job['done'] as List? ?? const []).join(', ');
    return Glass(
      borderRadius: 18,
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _Poster('${job['photo_url'] ?? ''}', w: 64),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(_title(job),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                            color: Colors.white,
                            fontSize: 15,
                            fontWeight: FontWeight.w700)),
                    const SizedBox(height: 4),
                    Text(_where(job),
                        style: const TextStyle(
                            color: AppColors.accent2,
                            fontSize: 13,
                            fontWeight: FontWeight.w600)),
                    const SizedBox(height: 6),
                    Text(
                      status == null
                          ? 'Jarayon ma\'lum emas — "Yangilash"ni bosing'
                          : [
                              p.stage,
                              if (p.quality.isNotEmpty) p.quality,
                              if (p.step.isNotEmpty) '(${p.step})',
                            ].join(' '),
                      style: const TextStyle(
                          color: Colors.white70, fontSize: 12.5),
                    ),
                    if (done.isNotEmpty) ...[
                      const SizedBox(height: 2),
                      Text('Tayyor: $done',
                          style: const TextStyle(
                              color: AppColors.success, fontSize: 12)),
                    ],
                  ],
                ),
              ),
            ],
          ),
          if (p.pct != null) ...[
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(4),
                    child: LinearProgressIndicator(
                      value: p.pct!.clamp(0.0, 1.0),
                      minHeight: 7,
                      backgroundColor: Colors.white12,
                      color: AppColors.accent2,
                    ),
                  ),
                ),
                const SizedBox(width: 10),
                Text('${(p.pct! * 100).toStringAsFixed(0)}%',
                    style: const TextStyle(
                        color: Colors.white,
                        fontWeight: FontWeight.w800,
                        fontSize: 13)),
              ],
            ),
          ],
          if (p.stats.isNotEmpty) ...[
            const SizedBox(height: 10),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final (k, v) in p.stats) _Stat(k, v),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

class _Stat extends StatelessWidget {
  final String k;
  final String v;
  const _Stat(this.k, this.v);

  @override
  Widget build(BuildContext context) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 6),
        decoration: BoxDecoration(
          color: Colors.white.withValues(alpha: 0.06),
          borderRadius: BorderRadius.circular(9),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(k,
                style: const TextStyle(color: Colors.white54, fontSize: 10.5)),
            Text(v,
                style: const TextStyle(
                    color: Colors.white,
                    fontSize: 12.5,
                    fontWeight: FontWeight.w700)),
          ],
        ),
      );
}

/// Encode log va GitHub run holati (so'ralgan paytdagi).
class _LiveCard extends StatelessWidget {
  final Map<String, dynamic>? status;
  final String statusError;
  final Map<String, dynamic>? github;
  final DateTime? fetchedAt;
  const _LiveCard({
    required this.status,
    required this.statusError,
    required this.github,
    required this.fetchedAt,
  });

  @override
  Widget build(BuildContext context) {
    final lines = (status?['lines'] as List? ?? const [])
        .map((e) => '$e')
        .where((e) => e.trim().isNotEmpty)
        .toList();
    final steps = (github?['steps'] as List? ?? const [])
        .whereType<Map<String, dynamic>>()
        .toList();
    final updated = _int(status?['updated_at']);
    return Glass(
      borderRadius: 18,
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.terminal_rounded,
                  color: AppColors.accent2, size: 18),
              const SizedBox(width: 6),
              const Expanded(
                child: Text('Encode log',
                    style: TextStyle(
                        color: Colors.white,
                        fontWeight: FontWeight.w700,
                        fontSize: 14)),
              ),
              if (updated > 0)
                Text(_ago(updated),
                    style:
                        const TextStyle(color: Colors.white54, fontSize: 11.5)),
            ],
          ),
          const SizedBox(height: 8),
          if (status == null)
            Text(
              statusError.isNotEmpty ? statusError : 'Log yo\'q',
              style: const TextStyle(color: Colors.white54, fontSize: 12.5),
            )
          else if (lines.isEmpty)
            const Text('Log hali bo\'sh',
                style: TextStyle(color: Colors.white54, fontSize: 12.5))
          else
            Container(
              width: double.infinity,
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: Colors.black.withValues(alpha: 0.45),
                borderRadius: BorderRadius.circular(10),
              ),
              child: SelectableText(
                lines.join('\n'),
                style: const TextStyle(
                  color: Color(0xFFB8F5C8),
                  fontFamily: 'monospace',
                  fontSize: 11,
                  height: 1.35,
                ),
              ),
            ),
          const SizedBox(height: 12),
          Row(
            children: [
              const Icon(Icons.play_circle_outline_rounded,
                  color: Colors.white70, size: 18),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  github == null
                      ? 'GitHub: ishlayotgan run yo\'q'
                      : 'GitHub run: ${_runStatus('${github!['status']}')}',
                  style: const TextStyle(color: Colors.white, fontSize: 13),
                ),
              ),
            ],
          ),
          if (steps.isNotEmpty) ...[
            const SizedBox(height: 6),
            for (final s in steps)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 2),
                child: Row(
                  children: [
                    _stepIcon('${s['status']}', '${s['conclusion']}'),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text('${s['name'] ?? ''}',
                          style: TextStyle(
                              color: s['status'] == 'in_progress'
                                  ? Colors.white
                                  : Colors.white60,
                              fontSize: 12.5,
                              fontWeight: s['status'] == 'in_progress'
                                  ? FontWeight.w700
                                  : FontWeight.w400)),
                    ),
                  ],
                ),
              ),
          ],
          if (fetchedAt != null) ...[
            const SizedBox(height: 8),
            Text(
              'So\'raldi: ${fetchedAt!.hour.toString().padLeft(2, '0')}:'
              '${fetchedAt!.minute.toString().padLeft(2, '0')}:'
              '${fetchedAt!.second.toString().padLeft(2, '0')}',
              style: const TextStyle(color: Colors.white38, fontSize: 11),
            ),
          ],
        ],
      ),
    );
  }

  static String _runStatus(String s) => switch (s) {
        'in_progress' => 'ishlayapti',
        'queued' => 'navbatda',
        _ => s,
      };

  static Widget _stepIcon(String status, String conclusion) {
    if (status == 'in_progress') {
      return const SizedBox(
        width: 14,
        height: 14,
        child: CircularProgressIndicator(
            strokeWidth: 2, color: AppColors.accent2),
      );
    }
    if (status != 'completed') {
      return const Icon(Icons.radio_button_unchecked,
          size: 14, color: Colors.white38);
    }
    return switch (conclusion) {
      'success' =>
        const Icon(Icons.check_circle, size: 14, color: AppColors.success),
      'skipped' =>
        const Icon(Icons.remove_circle_outline, size: 14, color: Colors.white38),
      _ => const Icon(Icons.cancel, size: 14, color: AppColors.danger),
    };
  }
}

/// Navbatdagi (yoki xato bilan to'xtagan) qism.
class _QueueTile extends StatelessWidget {
  final Map<String, dynamic> job;
  final bool error;
  const _QueueTile({required this.job, this.error = false});

  @override
  Widget build(BuildContext context) {
    final queued = _int(job['queued_at']);
    final err = '${job['error'] ?? ''}'.trim();
    return Glass(
      borderRadius: 16,
      padding: const EdgeInsets.all(10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _Poster('${job['photo_url'] ?? ''}'),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(_title(job),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        color: Colors.white,
                        fontSize: 14,
                        fontWeight: FontWeight.w700)),
                const SizedBox(height: 4),
                Text(_where(job),
                    style: const TextStyle(
                        color: AppColors.accent2,
                        fontSize: 12.5,
                        fontWeight: FontWeight.w600)),
                if (queued > 0) ...[
                  const SizedBox(height: 4),
                  Text('Navbatga qo\'yilgan: ${_ago(queued)}',
                      style: const TextStyle(
                          color: Colors.white54, fontSize: 11.5)),
                ],
                if (error && err.isNotEmpty) ...[
                  const SizedBox(height: 4),
                  Text(err,
                      maxLines: 3,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          color: AppColors.danger, fontSize: 11.5)),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}
