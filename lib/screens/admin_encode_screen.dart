// lib/screens/admin_encode_screen.dart — KODLASH NAVBATI (admin).
//
// TALAB (foydalanuvchi): "admin paneliga kodlash uchun navbatda turgan va
// encode qilinayotgan animelar bo'limini qo'sh. Yuqorida hozir
// kodlanayotgan: bo'lim nomi, nechanchi bo'lim va nechanchi qismligi va
// encode log statistikasi. Pastida navbatda turgan: bo'lim surati, bo'lim
// nomi, nechanchi bo'limligi va nechanchi qismligi."
// Keyin: "encode statistikani to'liq mayda detallarigacha aniq va real
// timeda ko'rsat ... videoni qancha daqiqa kodlangani va jami qancha
// daqiqaligi ham ko'rsatilsin".
//
// ── MA'LUMOT QAYERDAN ───────────────────────────────────────
//
//   * JONLI holat (~2 soniyada): runner (`tool/encode/run.py` ->
//     `StatusPin`) log kanalidagi QADALGAN `#arustatus` xabarini ~3
//     soniyada tahrirlaydi. Ilova uni adminning O'Z Telegram hisobi bilan
//     to'g'ridan-to'g'ri o'qiydi (`rust_tg_read_pinned`) — worker ham,
//     baza ham ishtirok etmaydi, ya'ni bepul.
//   * Admin hisobi kanalda bo'lmasa (yoki Telegram ulanmagan bo'lsa) —
//     worker orqali (`GET /api/encode/live`), 10 soniyada bir.
//   * Navbat — `GET /api/encode/admin` (bazadan bitta O'QISH): ekran
//     ochilganda, "Yangilash" bosilganda va kodlanayotgan qism
//     almashganda.
//   * GitHub run qadamlari — worker orqali, 30 soniyada bir.
//
// Ekran yopilishi bilan hamma so'rovlar to'xtaydi.

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

import '../services/auth_service.dart';
import '../services/telegram_service.dart';
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

  /// Jonli holat (qadalgan xabardan) — `null`: hali yo'q.
  Map<String, dynamic>? _status;
  String _statusError = '';
  Map<String, dynamic>? _github;
  String _logChat = '';

  /// Holat to'g'ridan-to'g'ri Telegram'dan o'qilyaptimi (aks holda worker).
  bool _direct = false;

  bool _loading = true;
  String? _error;
  DateTime? _at;

  Timer? _tick;
  bool _polling = false;
  int _ticks = 0;
  String _lastJob = '';

  Map<String, String> get _headers => {
        'Authorization': 'Bearer ${AuthService.instance.sessionToken}',
      };

  @override
  void initState() {
    super.initState();
    _load();
    _tick = Timer.periodic(const Duration(seconds: 2), (_) => _onTick());
  }

  @override
  void dispose() {
    _tick?.cancel();
    super.dispose();
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

  /// Navbat va worker'dagi jonli holat (GitHub bilan).
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
        _applyWorkerLive(res[1] ?? const {});
        _loading = false;
        _at = DateTime.now();
      });
      _lastJob = _jobKey(_status);
      unawaited(_readDirect());
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e is String ? e : 'Internet yo\'q';
        _loading = false;
      });
    }
  }

  void _applyWorkerLive(Map<String, dynamic> live) {
    _github = live['github'] is Map<String, dynamic>
        ? live['github'] as Map<String, dynamic>
        : null;
    _logChat = '${live['log_chat'] ?? ''}';
    // To'g'ridan-to'g'ri o'qilayotgan bo'lsa worker nusxasi (eskiroq)
    // ustidan yozilmaydi.
    if (!_direct) {
      _status = live['status'] is Map<String, dynamic>
          ? live['status'] as Map<String, dynamic>
          : null;
      _statusError = '${live['status_error'] ?? ''}';
    }
  }

  /// Qadalgan xabarni adminning o'z Telegram hisobi bilan o'qiydi.
  Future<bool> _readDirect() async {
    if (_logChat.isEmpty || !TelegramService.instance.authorized) {
      return false;
    }
    final j = await tgCall('rust_tg_read_pinned', arg: _logChat);
    if (!mounted) return false;
    final st = j['ok'] == true ? parseStatusPin('${j['text'] ?? ''}') : null;
    if (st == null) {
      if (_direct) setState(() => _direct = false);
      return false;
    }
    setState(() {
      _status = st;
      _statusError = '';
      _direct = true;
    });
    return true;
  }

  Future<void> _onTick() async {
    if (_polling || !mounted || _at == null) return;
    _polling = true;
    _ticks++;
    try {
      var ok = false;
      if (_logChat.isNotEmpty) ok = await _readDirect();
      if (!mounted) return;
      // Telegram'dan o'qib bo'lmasa — worker orqali (10 soniyada), GitHub
      // qadamlari esa har holda 30 soniyada.
      if ((!ok && _ticks % 5 == 0) || _ticks % 15 == 0) {
        final live = await _get('/api/encode/live');
        if (!mounted) return;
        setState(() => _applyWorkerLive(live ?? const {}));
      } else {
        // "N soniya oldin" yozuvlari yangilanib tursin.
        setState(() {});
      }
      // Runner boshqa qismga o'tdi — navbat ham yangilanadi.
      final key = _jobKey(_status);
      if (key != _lastJob) {
        _lastJob = key;
        final q = await _get('/api/encode/admin');
        if (!mounted) return;
        setState(() {
          _running = _list(q?['running']);
          _queue = _list(q?['queue']);
          _errors = _list(q?['errors']);
        });
      }
    } catch (_) {
      // Tarmoq yo'q — keyingi aylanishda.
    } finally {
      _polling = false;
    }
  }

  static String _jobKey(Map<String, dynamic>? st) => st == null
      ? ''
      : '${st['anime_id']}/${st['season_id']}/${st['epizod_id']}';

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
    final st = _status;
    final data = st?['data'] is Map<String, dynamic>
        ? st!['data'] as Map<String, dynamic>
        : const <String, dynamic>{};
    final active = st != null && _int(st['anime_id']) > 0;
    // Bazadagi "ishlayapti" yozuvi; bo'lmasa runner aytgan qism.
    Map<String, dynamic>? job;
    for (final j in _running) {
      if (_jobKey(st) == '${j['anime_id']}/${j['season_id']}/${j['epizod_id']}') {
        job = j;
      }
    }
    job ??= _running.isNotEmpty ? _running.first : null;
    return [
      if (_error != null) ...[
        Text(_error!, style: const TextStyle(color: AppColors.danger)),
        const SizedBox(height: 8),
      ],
      Row(
        children: [
          const _Label('HOZIR KODLANMOQDA'),
          const Spacer(),
          _LiveDot(direct: _direct, updatedMs: _int(st?['updated_at'])),
        ],
      ),
      const SizedBox(height: 8),
      if (job == null && !active)
        const _Empty('Hozir hech narsa kodlanmayapti')
      else ...[
        _JobHeader(job: job, status: st),
        const SizedBox(height: 10),
        _NowCard(status: st, data: data),
        const SizedBox(height: 10),
        if ((data['ladder'] as List? ?? const []).isNotEmpty) ...[
          _LadderCard(data: data),
          const SizedBox(height: 10),
        ],
        if (data['src'] is Map<String, dynamic>) ...[
          _SourceCard(src: data['src'] as Map<String, dynamic>),
          const SizedBox(height: 10),
        ],
      ],
      if (data['sys'] is Map<String, dynamic>) ...[
        _SysCard(
            sys: data['sys'] as Map<String, dynamic>,
            runS: _int(data['run_s'])),
        const SizedBox(height: 10),
      ],
      _LogCard(
        status: st,
        statusError: _statusError,
        github: _github,
      ),
      const SizedBox(height: 22),
      _Label('NAVBATDA (${_queue.length})'),
      const SizedBox(height: 8),
      if (_queue.isEmpty)
        const _Empty('Navbat bo\'sh')
      else
        for (var i = 0; i < _queue.length; i++) ...[
          _QueueTile(job: _queue[i], index: i + 1),
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
}

// ── QADALGAN XABARNI O'QISH ───────────────────────────────────
//
// Worker'dagi `parse_status_pin` bilan bir xil: `#arustatus`, keyin
// `kalit: qiymat` qatorlari (`data:` — JSON), `---` dan keyin log.

Map<String, dynamic>? parseStatusPin(String text) {
  final lines = const LineSplitter().convert(text);
  if (lines.isEmpty || lines.first.trim() != '#arustatus') return null;
  final head = <String, String>{};
  final log = <String>[];
  var body = false;
  for (final l in lines.skip(1)) {
    if (body) {
      log.add(l);
    } else if (l.trim() == '---') {
      body = true;
    } else {
      final i = l.indexOf(':');
      if (i > 0) head[l.substring(0, i).trim()] = l.substring(i + 1).trim();
    }
  }
  final job = (head['job'] ?? '').split('/').map(int.tryParse).toList();
  Map<String, dynamic> data = const {};
  try {
    final d = jsonDecode(head['data'] ?? '{}');
    if (d is Map<String, dynamic>) data = d;
  } catch (_) {}
  return {
    'run': head['run'] ?? '',
    'anime_id': job.isNotEmpty ? (job[0] ?? 0) : 0,
    'season_id': job.length > 1 ? (job[1] ?? 0) : 0,
    'epizod_id': job.length > 2 ? (job[2] ?? 0) : 0,
    'epizod_number': int.tryParse(head['num'] ?? '') ?? 0,
    'progress': head['progress'] ?? '',
    'updated_at': (int.tryParse(head['updated'] ?? '') ?? 0) * 1000,
    'lines': log,
    'data': data,
  };
}

// ── YORDAMCHILAR ──────────────────────────────────────────────

int _int(Object? v) => v is num ? v.toInt() : (int.tryParse('${v ?? 0}') ?? 0);

double? _dbl(Object? v) =>
    v is num ? v.toDouble() : double.tryParse('${v ?? ''}');

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

/// Soniya -> `1:02:03` yoki `02:03`.
String _clock(num? s) {
  if (s == null || s < 0) return '--:--';
  final v = s.toInt();
  final h = v ~/ 3600, m = (v % 3600) ~/ 60, sec = v % 60;
  String two(int x) => x.toString().padLeft(2, '0');
  return h > 0 ? '$h:${two(m)}:${two(sec)}' : '${two(m)}:${two(sec)}';
}

/// Soniya -> "12.5 daqiqa".
String _minutes(num? s) =>
    s == null || s < 0 ? '-' : '${(s / 60).toStringAsFixed(1)} daqiqa';

String _ago(int ms) {
  if (ms <= 0) return '';
  final s = (DateTime.now().millisecondsSinceEpoch - ms) ~/ 1000;
  if (s < 60) return '${s < 0 ? 0 : s} soniya oldin';
  if (s < 3600) return '${s ~/ 60} daqiqa oldin';
  return '${s ~/ 3600} soat oldin';
}

String _fmt(num? v, {int digits = 1, String suffix = ''}) =>
    v == null ? '-' : '${v.toStringAsFixed(digits)}$suffix';

String _thousands(int v) {
  final s = v.toString();
  final b = StringBuffer();
  for (var i = 0; i < s.length; i++) {
    if (i > 0 && (s.length - i) % 3 == 0) b.write(' ');
    b.write(s[i]);
  }
  return b.toString();
}

String _stage(String p) {
  final v = p.split('|');
  return switch (v.first) {
    'download' => 'Asl video yuklab olinmoqda',
    'upload' => 'Telegram\'ga yuklanmoqda${v.length > 1 ? ' (${v[1]})' : ''}',
    'enc' => 'Kodlanmoqda${v.length > 1 ? ' (${v[1]})' : ''}',
    'start' => 'Boshlanmoqda',
    'idle' => 'Kutmoqda',
    _ => p.isEmpty ? 'Noma\'lum' : p,
  };
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

/// Yashil nuqta — jonli (Telegram'dan), sariq — worker orqali / eskirgan.
class _LiveDot extends StatelessWidget {
  final bool direct;
  final int updatedMs;
  const _LiveDot({required this.direct, required this.updatedMs});

  @override
  Widget build(BuildContext context) {
    final age = updatedMs > 0
        ? (DateTime.now().millisecondsSinceEpoch - updatedMs) ~/ 1000
        : -1;
    final fresh = age >= 0 && age < 20;
    final color = direct && fresh ? AppColors.success : AppColors.gold;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 8,
          height: 8,
          decoration: BoxDecoration(color: color, shape: BoxShape.circle),
        ),
        const SizedBox(width: 6),
        Text(
          [
            direct ? 'Jonli' : 'Worker orqali',
            if (age >= 0) '${age}s',
          ].join(' · '),
          style: TextStyle(color: color, fontSize: 11.5),
        ),
      ],
    );
  }
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

class _Card extends StatelessWidget {
  final String title;
  final IconData icon;
  final Widget child;
  final Widget? trailing;
  const _Card(
      {required this.title,
      required this.icon,
      required this.child,
      this.trailing});

  @override
  Widget build(BuildContext context) => Glass(
        borderRadius: 18,
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(icon, color: AppColors.accent2, size: 18),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(title,
                      style: const TextStyle(
                          color: Colors.white,
                          fontWeight: FontWeight.w700,
                          fontSize: 14)),
                ),
                if (trailing != null) trailing!,
              ],
            ),
            const SizedBox(height: 10),
            child,
          ],
        ),
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
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(k,
                style: const TextStyle(color: Colors.white54, fontSize: 10.5)),
            const SizedBox(height: 1),
            Text(v,
                style: const TextStyle(
                    color: Colors.white,
                    fontSize: 12.5,
                    fontWeight: FontWeight.w700)),
          ],
        ),
      );
}

class _Bar extends StatelessWidget {
  final double value;
  final String label;
  const _Bar(this.value, this.label);

  @override
  Widget build(BuildContext context) => Row(
        children: [
          Expanded(
            child: ClipRRect(
              borderRadius: BorderRadius.circular(4),
              child: LinearProgressIndicator(
                value: value.clamp(0.0, 1.0),
                minHeight: 8,
                backgroundColor: Colors.white12,
                color: AppColors.accent2,
              ),
            ),
          ),
          const SizedBox(width: 10),
          Text(label,
              style: const TextStyle(
                  color: Colors.white,
                  fontWeight: FontWeight.w800,
                  fontSize: 13)),
        ],
      );
}

/// Qism: surat, bo'lim nomi, bo'lim va qism raqami, urinish.
class _JobHeader extends StatelessWidget {
  final Map<String, dynamic>? job;
  final Map<String, dynamic>? status;
  const _JobHeader({required this.job, required this.status});

  @override
  Widget build(BuildContext context) {
    final j = job ??
        {
          'anime_id': status?['anime_id'],
          'epizod_number': status?['epizod_number'],
        };
    final data = status?['data'] is Map<String, dynamic>
        ? status!['data'] as Map<String, dynamic>
        : const <String, dynamic>{};
    final started = _int(data['job_started']);
    return Glass(
      borderRadius: 18,
      padding: const EdgeInsets.all(12),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _Poster('${j['photo_url'] ?? ''}', w: 64),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(_title(j),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        color: Colors.white,
                        fontSize: 15,
                        fontWeight: FontWeight.w700)),
                if ('${j['anime_name'] ?? ''}'.isNotEmpty &&
                    '${j['nomi'] ?? ''}'.isNotEmpty)
                  Text('${j['anime_name']}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          color: Colors.white54, fontSize: 12)),
                const SizedBox(height: 4),
                Text(_where(j),
                    style: const TextStyle(
                        color: AppColors.accent2,
                        fontSize: 13,
                        fontWeight: FontWeight.w600)),
                const SizedBox(height: 6),
                Text(
                  status == null
                      ? 'Jarayon ma\'lum emas'
                      : _stage('${status!['progress'] ?? ''}'),
                  style: const TextStyle(color: Colors.white, fontSize: 12.5),
                ),
                if (started > 0)
                  Text(
                    'Qism boshlanganiga: '
                    '${_clock(DateTime.now().millisecondsSinceEpoch ~/ 1000 - started)}'
                    '${_int(data['attempt']) > 1 ? ' · ${data['attempt']}-urinish' : ''}',
                    style:
                        const TextStyle(color: Colors.white54, fontSize: 11.5),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Hozirgi bosqich: ffmpeg (kadrlar, kodlangan daqiqa / jami daqiqa,
/// tezlik, bitreyt, hajm, qolgan vaqt) yoki yuklash.
class _NowCard extends StatelessWidget {
  final Map<String, dynamic>? status;
  final Map<String, dynamic> data;
  const _NowCard({required this.status, required this.data});

  @override
  Widget build(BuildContext context) {
    final cur = data['cur'] is Map<String, dynamic>
        ? data['cur'] as Map<String, dynamic>
        : null;
    final xfer = data['xfer'] is Map<String, dynamic>
        ? data['xfer'] as Map<String, dynamic>
        : null;
    final src = data['src'] is Map<String, dynamic>
        ? data['src'] as Map<String, dynamic>
        : const <String, dynamic>{};
    if (cur == null && xfer == null) {
      return _Card(
        title: 'Joriy bosqich',
        icon: Icons.bolt_rounded,
        child: Text(
          status == null
              ? 'Holat hali kelmagan'
              : _stage('${status!['progress'] ?? ''}'),
          style: const TextStyle(color: Colors.white70, fontSize: 13),
        ),
      );
    }
    if (cur != null) {
      final pct = _dbl(cur['pct']) ?? 0;
      final out = _dbl(cur['out_s']);
      final dur = _dbl(cur['dur_s']);
      final frame = _int(cur['frame']);
      final frames = _int(src['frames']);
      return _Card(
        title: 'Kodlanmoqda: ${cur['q'] ?? ''}',
        icon: Icons.memory_rounded,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _Bar(pct / 100, '${pct.toStringAsFixed(1)}%'),
            const SizedBox(height: 10),
            // TALAB: "videoni qancha daqiqa kodlangani va jami qancha
            // daqiqaligi ko'rsatilsin".
            Text(
              'Kodlandi: ${_minutes(out)} / ${_minutes(dur)}  '
              '(${_clock(out)} / ${_clock(dur)})',
              style: const TextStyle(
                  color: Colors.white,
                  fontSize: 13.5,
                  fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 10),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                _Stat('Kadr',
                    frames > 0 ? '${_thousands(frame)} / ${_thousands(frames)}' : _thousands(frame)),
                _Stat('Kadr/s', _fmt(_dbl(cur['fps']))),
                _Stat('Tezlik', _fmt(_dbl(cur['speed']), digits: 2, suffix: 'x')),
                _Stat('Bitreyt', _fmt(_dbl(cur['bitrate_kbps']), digits: 0, suffix: ' kb/s')),
                _Stat('Hajm', _fmt(_dbl(cur['size_mb']), suffix: ' MB')),
                _Stat('Taxminiy hajm', _fmt(_dbl(cur['est_mb']), digits: 0, suffix: ' MB')),
                _Stat('O\'tdi', _clock(_dbl(cur['elapsed']))),
                _Stat('Qoldi', '~${_clock(_dbl(cur['eta']))}'),
                _Stat('Kvantizator (q)', _fmt(_dbl(cur['qp']))),
                _Stat('Tashlangan / takror',
                    '${_int(cur['drop'])} / ${_int(cur['dup'])}'),
              ],
            ),
          ],
        ),
      );
    }
    final x = xfer!;
    final pct = _dbl(x['pct']) ?? 0;
    return _Card(
      title: '${x['kind'] ?? 'Yuklash'}',
      icon: Icons.swap_vert_rounded,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _Bar(pct / 100, '${pct.toStringAsFixed(1)}%'),
          const SizedBox(height: 10),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              _Stat('Hajm',
                  '${_fmt(_dbl(x['cur_mb']))} / ${_fmt(_dbl(x['total_mb']), suffix: ' MB')}'),
              _Stat('Tezlik', _fmt(_dbl(x['mbps']), digits: 2, suffix: ' MB/s')),
              _Stat('O\'tdi', _clock(_dbl(x['elapsed']))),
              _Stat('Qoldi', '~${_clock(_dbl(x['eta']))}'),
            ],
          ),
        ],
      ),
    );
  }
}

/// Sifatlar: har biri kutmoqda / kodlanmoqda / yuklanmoqda / tayyor.
class _LadderCard extends StatelessWidget {
  final Map<String, dynamic> data;
  const _LadderCard({required this.data});

  @override
  Widget build(BuildContext context) {
    final rows = (data['ladder'] as List? ?? const [])
        .whereType<Map<String, dynamic>>()
        .toList();
    final done = rows.where((r) => r['state'] == 'done').length;
    return _Card(
      title: 'Sifatlar',
      icon: Icons.layers_rounded,
      trailing: Text('$done/${rows.length} tayyor',
          style: const TextStyle(color: Colors.white54, fontSize: 12)),
      child: Column(
        children: [
          for (final r in rows)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Row(
                children: [
                  _stateIcon('${r['state']}'),
                  const SizedBox(width: 8),
                  SizedBox(
                    width: 52,
                    child: Text('${r['q']}',
                        style: const TextStyle(
                            color: Colors.white,
                            fontWeight: FontWeight.w700,
                            fontSize: 13)),
                  ),
                  Expanded(
                    child: Text(
                      [
                        _stateText('${r['state']}'),
                        if (r['crf'] != null) 'CRF ${r['crf']}',
                        if (r['size_mb'] != null) '${r['size_mb']} MB',
                        if (r['kbps'] != null) '${r['kbps']} kb/s',
                        if (r['enc_s'] != null) _clock(_dbl(r['enc_s'])),
                      ].join(' · '),
                      style: const TextStyle(
                          color: Colors.white70, fontSize: 12),
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }

  static String _stateText(String s) => switch (s) {
        'enc' => 'kodlanmoqda',
        'upload' => 'yuklanmoqda',
        'done' => 'tayyor',
        _ => 'kutmoqda',
      };

  static Widget _stateIcon(String s) => switch (s) {
        'done' =>
          const Icon(Icons.check_circle, size: 16, color: AppColors.success),
        'enc' || 'upload' => const SizedBox(
            width: 16,
            height: 16,
            child: CircularProgressIndicator(
                strokeWidth: 2, color: AppColors.accent2),
          ),
        _ => const Icon(Icons.radio_button_unchecked,
            size: 16, color: Colors.white38),
      };
}

/// Asl video: o'lcham, davomiylik, hajm, bitreyt, kadrlar.
class _SourceCard extends StatelessWidget {
  final Map<String, dynamic> src;
  const _SourceCard({required this.src});

  @override
  Widget build(BuildContext context) => _Card(
        title: 'Asl video',
        icon: Icons.movie_rounded,
        child: Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            _Stat('O\'lcham',
                '${_int(src['w']) > 0 ? '${src['w']}×' : ''}${src['h']}p'),
            _Stat('Davomiyligi',
                '${_minutes(_dbl(src['dur_s']))} (${_clock(_dbl(src['dur_s']))})'),
            _Stat('Hajm', _fmt(_dbl(src['mb']), suffix: ' MB')),
            _Stat('Bitreyt', _fmt(_dbl(src['kbps']), digits: 0, suffix: ' kb/s')),
            if (src['fps'] != null) _Stat('Kadr/s', _fmt(_dbl(src['fps']), digits: 2)),
            if (_int(src['frames']) > 0)
              _Stat('Jami kadr', _thousands(_int(src['frames']))),
            if ('${src['codec'] ?? ''}'.isNotEmpty)
              _Stat('Kodek', '${src['codec']}'),
          ],
        ),
      );
}

/// Runner kompyuteri: CPU, RAM, disk.
class _SysCard extends StatelessWidget {
  final Map<String, dynamic> sys;
  final int runS;
  const _SysCard({required this.sys, required this.runS});

  @override
  Widget build(BuildContext context) {
    final used = _int(sys['ram_used_mb']);
    final total = _int(sys['ram_total_mb']);
    return _Card(
      title: 'Kompyuter (GitHub runner)',
      icon: Icons.developer_board_rounded,
      child: Wrap(
        spacing: 8,
        runSpacing: 8,
        children: [
          _Stat('CPU', _fmt(_dbl(sys['cpu']), suffix: '%')),
          _Stat('Yadro', '${sys['cores'] ?? '-'}'),
          _Stat('Yuklama', _fmt(_dbl(sys['load']), digits: 2)),
          _Stat('RAM', total > 0 ? '$used / $total MB' : '-'),
          _Stat('Bo\'sh disk', _fmt(_dbl(sys['disk_free_gb']), suffix: ' GB')),
          _Stat('Run vaqti', _clock(runS)),
        ],
      ),
    );
  }
}

/// Encode log va GitHub run qadamlari.
class _LogCard extends StatelessWidget {
  final Map<String, dynamic>? status;
  final String statusError;
  final Map<String, dynamic>? github;
  const _LogCard({
    required this.status,
    required this.statusError,
    required this.github,
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
    return _Card(
      title: 'Encode log',
      icon: Icons.terminal_rounded,
      trailing: updated > 0
          ? Text(_ago(updated),
              style: const TextStyle(color: Colors.white54, fontSize: 11.5))
          : null,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
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
          Text(
            github == null
                ? 'GitHub: ishlayotgan run yo\'q'
                : 'GitHub run: ${_runStatus('${github!['status']}')}',
            style: const TextStyle(color: Colors.white, fontSize: 13),
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
      'skipped' => const Icon(Icons.remove_circle_outline,
          size: 14, color: Colors.white38),
      _ => const Icon(Icons.cancel, size: 14, color: AppColors.danger),
    };
  }
}

/// Navbatdagi (yoki xato bilan to'xtagan) qism.
class _QueueTile extends StatelessWidget {
  final Map<String, dynamic> job;
  final bool error;
  final int index;
  const _QueueTile({required this.job, this.error = false, this.index = 0});

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
                Text(
                    '${index > 0 ? '$index. ' : ''}${_title(job)}',
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
