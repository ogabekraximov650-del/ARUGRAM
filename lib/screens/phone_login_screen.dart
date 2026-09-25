// lib/screens/phone_login_screen.dart — TELEGRAM ORQALI KIRISH
//
// TALAB (foydalanuvchi): "profil sahifasiga o'tganda xuddi
// Telegramdagidek raqam yozadigan, keyin kod yozadigan va agar
// bo'lsa ikki bosqichli parol yozadigan oyna tizimi bo'lsin.
// Raqam yozadigan oynada faqat + bo'lsin, foydalanuvchi qolganini
// o'zi yozsin".
//
// ── QANDAY ISHLAYDI ────────────────────────────────────────────
//
//   1. Raqam -> Telegram kod yuboradi (Telegram ilovasiga yoki SMS).
//   2. Kod -> (yoqilgan bo'lsa) ikki bosqichli parol.
//   3. Telegram hisobi ulangach ilova foydalanuvchi NOMIDAN botga
//      `/start <token>` yuboradi (`rust_tg_start_bot`) — foydalanuvchi
//      talabi: hisob avvalgidek bot orqali tasdiqlanadi. Bot xabarni
//      kim yuborganini Telegram'ning o'zidan biladi va sessiya ochadi.
//
// Natijada bitta kirish bilan ikkisi bo'ladi: ilova hisobi ochiladi
// VA videolar uchun Telegram ulanadi (`telegram_service.dart`).
//
// [connectOnly] — ilovaga allaqachon kirilgan, faqat Telegram
// ulanadi (3-qadam yo'q).
//
// [gate] — ilovaning o'zi shu oyna (`AuthGate`), kirilmaguncha
// boshqa hech narsa ochilmaydi.
//
// Ekran `true` qaytaradi — kirildi.

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../services/auth_service.dart';
import '../services/telegram_service.dart';
import '../widgets/tg_countries.dart';

enum _Step { phone, code, password, qr, finishing }

class PhoneLoginScreen extends StatefulWidget {
  final bool connectOnly;

  /// Ilovaning O'ZI shu oyna (`AuthGate`): orqaga tugmasi yo'q va
  /// kirilgach sahifa yopilmaydi — ilova o'zi ochiladi.
  final bool gate;
  const PhoneLoginScreen(
      {super.key, this.connectOnly = false, this.gate = false});

  @override
  State<PhoneLoginScreen> createState() => _PhoneLoginScreenState();
}

class _PhoneLoginScreenState extends State<PhoneLoginScreen> {
  /// To'liq raqam (`+998901234567`) — Telegram'ga shu yuboriladi.
  /// Ekranda esa davlat kodi [_cc] va raqam [_num] alohida.
  final _phone = TextEditingController(text: '+998');
  final _cc = TextEditingController(text: '998');
  final _num = TextEditingController();
  TgCountry? _country = countryByIso('UZ');

  /// Xato bo'lganda kod kataklari chayqaladi (`_Shake`).
  int _shake = 0;

  void _syncPhone() {
    _phone.text =
        '+${_cc.text}${_num.text.replaceAll(RegExp(r'[^0-9]'), '')}';
  }

  void _onCodeChanged(String v) {
    _ccTouched = true;
    setState(() => _country = countryByCode(v, prefer: _country));
    // Raqam shabloni yangi davlatga moslanadi.
    final d = _num.text.replaceAll(RegExp(r'[^0-9]'), '');
    _num.text = _country?.format(d) ?? d;
    _syncPhone();
  }

  /// Saqlangan to'liq raqamni davlat va raqamga ajratadi.
  void _applyFull(String phone) {
    final (c, rest) = splitPhone(phone.replaceAll(RegExp(r'[^0-9]'), ''));
    if (c == null) return;
    _ccTouched = true;
    _country = c;
    _cc.text = c.code;
    _num.text = c.format(rest);
    _syncPhone();
  }
  final _code = TextEditingController();
  final _password = TextEditingController();
  final _focus = FocusNode();

  _Step _step = _Step.phone;
  bool _busy = false;
  bool _showPassword = false;
  String? _error;
  String _hint = '';
  bool _available = true;

  // ── KOD QAYERGA KETDI VA QAYTA YUBORISH ─────────────────────
  //
  // TOPILGAN XATO (foydalanuvchi: "kod yuborildi deyapti, lekin
  // umuman kelmayapti"): oyna doim "Telegram chatida" deb yozardi,
  // Telegram esa kodni SMS, qo'ng'iroq yoki emailga ham yuboradi.
  // Endi Telegram aytgan joy ko'rsatiladi va kutish vaqti o'tgach
  // "Kodni qayta yuborish" (keyingi usul) tugmasi chiqadi.
  // ── TELEGRAM AYTGAN KUTISH ──────────────────────────────────
  //
  // TALAB (foydalanuvchi): "qancha kutish kerakligini aniq
  // ko'rsatsin". Telegram vaqtni aytsa (FLOOD_WAIT) — teskari sanoq,
  // tugaguncha kod so'rab bo'lmaydi. Aytmasa — xato matnida
  // shunday deyiladi.
  int _waitLeft = 0;
  Timer? _waitTimer;

  void _setWait(int secs) {
    _waitTimer?.cancel();
    _waitLeft = secs;
    if (secs > 0) {
      _waitTimer = Timer.periodic(const Duration(seconds: 1), (t) {
        if (!mounted) return t.cancel();
        setState(() => _waitLeft--);
        if (_waitLeft <= 0) {
          t.cancel();
          setState(() => _error = null);
        }
      });
    }
    if (mounted) setState(() {});
  }

  /// `1:05:09` yoki `4:09`.
  static String _clock(int s) {
    final h = s ~/ 3600, m = (s % 3600) ~/ 60, sec = s % 60;
    String two(int v) => v.toString().padLeft(2, '0');
    return h > 0 ? '$h:${two(m)}:${two(sec)}' : '$m:${two(sec)}';
  }

  Map<String, dynamic> _sent = const {};
  Timer? _resendTimer;
  int _resendLeft = 0;

  void _setSent(Object? sent) {
    _sent = (sent is Map) ? sent.cast<String, dynamic>() : const {};
    final timeout = (_sent['timeout'] as num?)?.toInt() ?? 0;
    final at = (_sent['at'] as num?)?.toInt() ?? 0;
    final passed = at > 0
        ? (DateTime.now().millisecondsSinceEpoch - at) ~/ 1000
        : 0;
    // Telegram kutish vaqtini aytmagan bo'lsa ham 30 soniyadan
    // keyin qayta so'rash mumkin bo'lsin.
    _resendLeft = ((timeout > 0 ? timeout : 30) - passed).clamp(0, 600);
    _resendTimer?.cancel();
    if (_resendLeft > 0) {
      _resendTimer = Timer.periodic(const Duration(seconds: 1), (t) {
        if (!mounted) return t.cancel();
        setState(() => _resendLeft--);
        if (_resendLeft <= 0) t.cancel();
      });
    }
    if (mounted) setState(() {});
  }

  int get _codeLength {
    final n = (_sent['length'] as num?)?.toInt() ?? 0;
    return n >= 4 && n <= 8 ? n : 5;
  }

  /// Kod qayerga yuborilgani — odam tushunadigan qilib.
  String _whereSent() {
    final p = _phone.text;
    final pattern = '${_sent['pattern'] ?? ''}';
    switch (_sent['via']) {
      case 'sms':
      case 'sms_word':
      case 'sms_phrase':
        return 'Telegram $p raqamiga SMS orqali kod yubordi.';
      case 'call':
        return 'Telegram $p raqamiga qo\'ng\'iroq qiladi — kodni aytib beradi.';
      case 'flash_call':
      case 'missed_call':
        return 'Telegram $p raqamiga qo\'ng\'iroq qiladi. Kod — qo\'ng\'iroq '
            'qilgan raqamning oxirgi $_codeLength ta raqami'
            '${pattern.isEmpty ? '' : ' ($pattern...)'}.';
      case 'email':
        return 'Kod emailingizga yuborildi${pattern.isEmpty ? '' : ': $pattern'}.';
      case 'fragment':
        return 'Kod Fragment orqali yuborildi${pattern.isEmpty ? '' : ' ($pattern)'}.';
      default:
        return 'Telegram $p raqamiga kirish kodini yubordi.\n'
            'Kod Telegram ilovasidagi "Telegram" chatida (boshqa '
            'qurilmadagi Telegram\'da ham ko\'rinadi).';
    }
  }

  String get _nextLabel => switch (_sent['next']) {
        'sms' => 'SMS orqali yuborish',
        'call' => 'Qo\'ng\'iroq orqali yuborish',
        'flash_call' || 'missed_call' => 'Qo\'ng\'iroq orqali yuborish',
        _ => 'Kodni qayta yuborish',
      };

  // ── QR ORQALI KIRISH ────────────────────────────────────────
  //
  // Kod kelmasa ham kirish mumkin bo'lsin (Cherrygram kabi): QR
  // boshqa qurilmadagi Telegram'da Sozlamalar → Qurilmalar →
  // "Qurilmani ulash" bilan skanerlanadi. Tasdiqlanganini yadro
  // sezadi (`updateLoginToken`) — shunda token qayta so'raladi va
  // kirish yakunlanadi. Token muddati tugasa QR yangilanadi.
  String _qrUrl = '';
  DateTime _qrUntil = DateTime.fromMillisecondsSinceEpoch(0);
  Timer? _qrTimer;
  bool _qrBusy = false;

  void _startQr() {
    _go(_Step.qr);
    _qrUrl = '';
    unawaited(_qrRefresh());
    _qrTimer?.cancel();
    _qrTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted || _step != _Step.qr) return;
      if (_tg.qrAccepted() || DateTime.now().isAfter(_qrUntil)) {
        unawaited(_qrRefresh());
      }
    });
  }

  void _stopQr() {
    _qrTimer?.cancel();
    _qrTimer = null;
  }

  Future<void> _qrRefresh() async {
    if (_qrBusy) return;
    _qrBusy = true;
    try {
      final j = await _tg.qrToken();
      if (!mounted || _step != _Step.qr) return;
      if (j['ok'] == true) {
        _stopQr();
        await _finish();
        return;
      }
      if (j['password'] == true) {
        _stopQr();
        _hint = '${j['hint'] ?? ''}';
        _go(_Step.password);
        return;
      }
      final err = j['error'];
      if (err is String && err.isNotEmpty) {
        // Bir oz kutib qayta uriniladi.
        _qrUntil = DateTime.now().add(const Duration(seconds: 5));
        setState(() => _error = err);
        return;
      }
      final left = (j['expires'] as num?)?.toInt() ?? 30;
      setState(() {
        _error = null;
        _qrUrl = '${j['url'] ?? ''}';
        _qrUntil = DateTime.now().add(Duration(seconds: left));
      });
    } finally {
      _qrBusy = false;
    }
  }

  Future<void> _resend() async {
    if (_busy || _resendLeft > 0 || _waitLeft > 0) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final r = await _tg.resendCode();
    if (!mounted) return;
    if (r.needPassword) {
      _hint = r.hint;
      setState(() => _busy = false);
      _go(_Step.password);
      return;
    }
    if (r.loggedIn) return _finish();
    if (r.error != null) return _fail(r.error, wait: r.wait);
    setState(() => _busy = false);
    _code.clear();
    _setSent(r.sent);
  }

  TelegramService get _tg => TelegramService.instance;

  @override
  void initState() {
    super.initState();
    _check();
  }

  Future<void> _check() async {
    // ── SAQLANGAN BOSQICH (tarmoqsiz, darhol) ─────────────────
    // Ilova kod kutilayotgan paytda yopilgan bo'lsa — to'g'ridan-
    // to'g'ri kod (yoki parol) oynasi ochiladi.
    _restoreStage();
    final ok = await _tg.refreshConfig();
    if (!mounted) return;
    // Telegram allaqachon ulangan (masalan oldingi urinish 3-qadamda
    // uzilgan) — raqamni qayta so'rashning hojati yo'q.
    if (_tg.authorized) {
      _finish();
      return;
    }
    setState(() => _available = ok);
    unawaited(_detectCountry());
  }

  /// Foydalanuvchi davlatni o'zi tanlagan / kodni o'zgartirgan.
  bool _ccTouched = false;

  /// IP manzil bo'yicha davlat kodi (Telegram'dagidek) — masalan
  /// O'zbekistonda `+998`. Foydalanuvchi uni istalgan payt
  /// o'zgartira oladi; o'zgartirgan yoki raqam yozib qo'ygan bo'lsa
  /// tegilmaydi.
  Future<void> _detectCountry() async {
    final iso = await _tg.nearestCountry();
    if (!mounted || iso == null || _ccTouched || _step != _Step.phone) return;
    final c = countryByIso(iso);
    if (c == null || _num.text.isNotEmpty) return;
    setState(() {
      _country = c;
      _cc.text = c.code;
    });
    _syncPhone();
  }

  void _restoreStage() {
    final st = _tg.loginState();
    _setWait((st['wait'] as num?)?.toInt() ?? 0);
    final phone = (st['phone'] as String?) ?? '';
    if (phone.isNotEmpty) _applyFull(phone);
    switch (st['stage']) {
      case 'code':
        _setSent(st['sent']);
        _go(_Step.code);
      case 'password':
        _hint = (st['hint'] as String?) ?? '';
        _go(_Step.password);
    }
  }

  /// Raqam bosqichiga qaytish ("Raqamni o'zgartirish").
  void _backToPhone() {
    _tg.resetLogin();
    _code.clear();
    _password.clear();
    _go(_Step.phone);
  }

  @override
  void dispose() {
    _waitTimer?.cancel();
    _qrTimer?.cancel();
    _resendTimer?.cancel();
    _phone.dispose();
    _cc.dispose();
    _num.dispose();
    _code.dispose();
    _password.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _go(_Step s) {
    setState(() {
      _step = s;
      _error = null;
    });
    // Yangi maydonga klaviatura o'zi ochilsin.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && s != _Step.finishing) _focus.requestFocus();
    });
  }

  void _fail(String? e, {int wait = 0}) {
    if (!mounted) return;
    if (wait > 0) _setWait(wait);
    if (_step == _Step.code || _step == _Step.password) {
      HapticFeedback.heavyImpact();
      _shake++;
      if (_step == _Step.code) _code.clear();
    }
    setState(() {
      _busy = false;
      _error = e;
    });
  }

  Future<void> _submit() async {
    if (_busy) return;
    switch (_step) {
      case _Step.qr:
        return;
      case _Step.phone:
        if (_waitLeft > 0) return;
        _syncPhone();
        if (_phone.text.length < 8) {
          _fail('Raqamni to\'liq kiriting');
          return;
        }
        setState(() {
          _busy = true;
          _error = null;
        });
        final r = await _tg.requestCode(_phone.text);
        if (!mounted) return;
        if (r.error != null) return _fail(r.error, wait: r.wait);
        // Kirish tokeni tanildi (Cherrygram kabi) — kodsiz kiritildi
        // yoki darhol parol so'raldi.
        if (r.needPassword) {
          _hint = r.hint;
          setState(() => _busy = false);
          _go(_Step.password);
          return;
        }
        if (r.loggedIn) return _finish();
        setState(() => _busy = false);
        _setSent(r.sent);
        _go(_Step.code);
      case _Step.code:
        final code = _code.text.trim();
        if (code.isEmpty) return;
        setState(() {
          _busy = true;
          _error = null;
        });
        final r = await _tg.signIn(code);
        if (!mounted) return;
        if (r.needPassword) {
          _hint = r.hint;
          setState(() => _busy = false);
          _go(_Step.password);
          return;
        }
        if (!r.done) return _fail(r.error, wait: r.wait);
        _finish();
      case _Step.password:
        if (_password.text.isEmpty) return;
        setState(() {
          _busy = true;
          _error = null;
        });
        final r = await _tg.checkPassword(_password.text);
        if (!mounted) return;
        if (!r.done) {
          _password.clear();
          return _fail(r.error ?? 'Parol noto\'g\'ri', wait: r.wait);
        }
        _finish();
      case _Step.finishing:
        _finish();
    }
  }

  /// Telegram ulandi. Ilova hisobi ham kerak bo'lsa — bot orqali
  /// sessiya ochiladi.
  Future<void> _finish() async {
    if (widget.connectOnly || AuthService.instance.isLoggedIn) {
      // Darvoza rejimida ilova o'zi ochiladi (`AuthGate` tinglaydi).
      if (mounted && !widget.gate) Navigator.of(context).pop(true);
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
      _step = _Step.finishing;
    });
    final req = await AuthService.instance.start();
    if (!mounted) return;
    if (req == null) return _fail('Server bilan bog\'lanib bo\'lmadi');
    final err = await _tg.startBot(req.token);
    if (!mounted) return;
    if (err != null) return _fail(err);
    // Bot xabarni bir-ikki soniyada oladi.
    for (var i = 0; i < 40; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 750));
      if (!mounted) return;
      final st = await AuthService.instance.check(req.token);
      if (!mounted) return;
      if (st == LoginStatus.ok) {
        if (!widget.gate) Navigator.of(context).pop(true);
        return;
      }
      if (st == LoginStatus.expired) break;
    }
    _fail('Kirish tasdiqlanmadi — qayta urinib ko\'ring');
  }

  // ═══════════════════════════════════════════════════════════
  //  KO'RINISH — HAQIQIY TELEGRAM KIRISH OYNASIDEK
  // ═══════════════════════════════════════════════════════════
  //
  // TALAB (foydalanuvchi): "Telegram accountga kirish oynasini
  // huddi haqiqiy telegram uisidek qilib ber". Telegram Android'ning
  // tungi ko'rinishi: to'q ko'k fon, chapga tekislangan sarlavha,
  // davlat tanlash qatori, kod va raqam alohida maydonlarda, kod
  // uchun raqam kataklari, pastki o'ngda ko'k dumaloq tugma,
  // bosqichlar orasida yon tomonga siljish.

  @override
  Widget build(BuildContext context) {
    final showFab = _step != _Step.qr &&
        !(_step == _Step.finishing && _error == null);
    final canBack = !((widget.gate && _step == _Step.phone) ||
        _step == _Step.finishing);
    return Scaffold(
      backgroundColor: _Tg.bg,
      appBar: AppBar(
        backgroundColor: _Tg.bg,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        automaticallyImplyLeading: false,
        iconTheme: const IconThemeData(color: Colors.white),
        leading: canBack
            ? IconButton(
                icon: const Icon(Icons.arrow_back),
                onPressed: _busy ? null : _back,
              )
            : null,
      ),
      floatingActionButton: AnimatedScale(
        scale: showFab ? 1 : 0,
        duration: const Duration(milliseconds: 200),
        curve: Curves.easeOutBack,
        child: FloatingActionButton(
          onPressed: _busy || !_available || !showFab ? null : _submit,
          backgroundColor: _Tg.accent,
          elevation: 2,
          shape: const CircleBorder(),
          child: AnimatedSwitcher(
            duration: const Duration(milliseconds: 180),
            child: _busy
                ? const SizedBox(
                    key: ValueKey('busy'),
                    width: 24,
                    height: 24,
                    child: CircularProgressIndicator(
                        strokeWidth: 2.4, color: Colors.white),
                  )
                : const Icon(Icons.arrow_forward,
                    key: ValueKey('go'), color: Colors.white),
          ),
        ),
      ),
      body: SafeArea(
        top: false,
        child: AnimatedSwitcher(
          duration: const Duration(milliseconds: 280),
          switchInCurve: Curves.easeOutCubic,
          switchOutCurve: Curves.easeInCubic,
          transitionBuilder: (child, anim) => FadeTransition(
            opacity: anim,
            child: SlideTransition(
              position: Tween(
                begin: const Offset(0.18, 0),
                end: Offset.zero,
              ).animate(anim),
              child: child,
            ),
          ),
          child: ListView(
            key: ValueKey(_step),
            padding: const EdgeInsets.fromLTRB(24, 8, 24, 120),
            children: [
              _stepBody(),
              if (_waitLeft > 0) ...[
                const SizedBox(height: 20),
                Text(
                  'Qayta urinish mumkin: ${_clock(_waitLeft)} dan keyin',
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                      color: Color(0xFFF5A623),
                      fontSize: 15,
                      fontWeight: FontWeight.w600),
                ),
              ],
              if (_error != null) ...[
                const SizedBox(height: 16),
                Text(
                  _error!,
                  textAlign: TextAlign.center,
                  style: const TextStyle(color: _Tg.red, fontSize: 14),
                ),
              ],
              if (!_available) ...[
                const SizedBox(height: 16),
                const Text(
                  'Telegram orqali kirish hozircha yoqilmagan.',
                  textAlign: TextAlign.center,
                  style: TextStyle(color: _Tg.hint, fontSize: 14),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  void _back() {
    if (_step == _Step.qr) {
      _stopQr();
      _go(_Step.phone);
    } else if (_step == _Step.code || _step == _Step.password) {
      _backToPhone();
    } else {
      Navigator.of(context).maybePop();
    }
  }

  Widget _header(IconData icon, String title, String subtitle,
      {bool center = false}) {
    final align = center ? CrossAxisAlignment.center : CrossAxisAlignment.start;
    final ta = center ? TextAlign.center : TextAlign.start;
    return Column(
      crossAxisAlignment: align,
      children: [
        Center(
          child: Container(
            width: 104,
            height: 104,
            decoration: const BoxDecoration(
              shape: BoxShape.circle,
              gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [Color(0xFF6CC3F6), _Tg.accentDark],
              ),
            ),
            child: Icon(icon, color: Colors.white, size: 52),
          ),
        ),
        const SizedBox(height: 26),
        SizedBox(
          width: double.infinity,
          child: Text(title,
              textAlign: ta,
              style: const TextStyle(
                  color: Colors.white,
                  fontSize: 22,
                  fontWeight: FontWeight.w600)),
        ),
        const SizedBox(height: 10),
        SizedBox(
          width: double.infinity,
          child: Text(subtitle,
              textAlign: ta,
              style: const TextStyle(
                  color: _Tg.hint, fontSize: 15, height: 1.4)),
        ),
        const SizedBox(height: 30),
      ],
    );
  }

  InputDecoration _underline({String? label, String? hint, Widget? suffix}) {
    return InputDecoration(
      labelText: label,
      hintText: hint,
      hintStyle: const TextStyle(color: _Tg.faint),
      labelStyle: const TextStyle(color: _Tg.hint),
      floatingLabelStyle: const TextStyle(color: _Tg.accent),
      suffixIcon: suffix,
      isDense: true,
      contentPadding: const EdgeInsets.symmetric(vertical: 10),
      enabledBorder: const UnderlineInputBorder(
          borderSide: BorderSide(color: _Tg.line)),
      focusedBorder: const UnderlineInputBorder(
          borderSide: BorderSide(color: _Tg.accent, width: 2)),
      disabledBorder: const UnderlineInputBorder(
          borderSide: BorderSide(color: _Tg.line)),
    );
  }

  Widget _link(String text, VoidCallback? onTap) {
    return TextButton(
      onPressed: onTap,
      style: TextButton.styleFrom(
        foregroundColor: _Tg.accent,
        disabledForegroundColor: _Tg.faint,
        textStyle: const TextStyle(fontSize: 15, fontWeight: FontWeight.w500),
      ),
      child: Text(text),
    );
  }

  /// Raqam `+998 90 123 45 67` ko'rinishida.
  String get _prettyPhone {
    final c = _country;
    final n = _num.text.replaceAll(RegExp(r'[^0-9]'), '');
    return '+${_cc.text} ${c == null ? n : c.format(n)}'.trim();
  }

  Widget _phoneBody() {
    const big = TextStyle(color: Colors.white, fontSize: 18);
    final c = _country;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const SizedBox(height: 16),
        const Text('Telefon raqamingiz',
            style: TextStyle(
                color: Colors.white, fontSize: 22, fontWeight: FontWeight.w600)),
        const SizedBox(height: 10),
        const Text(
            'Davlatni tanlang va Telegram hisobingiz ulangan telefon '
            'raqamini kiriting.',
            style: TextStyle(color: _Tg.hint, fontSize: 15, height: 1.4)),
        const SizedBox(height: 30),
        // ── Davlat ──
        InkWell(
          onTap: _busy || !_available ? null : _pickCountry,
          child: Container(
            padding: const EdgeInsets.symmetric(vertical: 12),
            decoration: const BoxDecoration(
                border: Border(bottom: BorderSide(color: _Tg.line))),
            child: Row(
              children: [
                if (c != null) ...[
                  Text(c.flag, style: const TextStyle(fontSize: 22)),
                  const SizedBox(width: 12),
                ],
                Expanded(
                  child: Text(
                    c?.name ??
                        (_cc.text.isEmpty ? 'Davlatni tanlang' : 'Noto\'g\'ri kod'),
                    style: big.copyWith(
                        color: c == null ? _Tg.hint : Colors.white),
                  ),
                ),
                const Icon(Icons.chevron_right, color: _Tg.hint),
              ],
            ),
          ),
        ),
        const SizedBox(height: 14),
        // ── Kod + raqam ──
        Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            SizedBox(
              width: 74,
              child: TextField(
                controller: _cc,
                enabled: !_busy && _available,
                keyboardType: TextInputType.phone,
                inputFormatters: [
                  FilteringTextInputFormatter.digitsOnly,
                  LengthLimitingTextInputFormatter(4),
                ],
                style: big,
                cursorColor: _Tg.accent,
                onChanged: _onCodeChanged,
                decoration: _underline().copyWith(
                  prefixText: '+',
                  prefixStyle: big,
                ),
              ),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: TextField(
                controller: _num,
                focusNode: _focus,
                autofocus: true,
                enabled: !_busy && _available,
                keyboardType: TextInputType.phone,
                inputFormatters: [_NumberFormatter(() => _country)],
                style: big.copyWith(letterSpacing: 0.6),
                cursorColor: _Tg.accent,
                onChanged: (_) => _syncPhone(),
                onSubmitted: (_) => _submit(),
                decoration: _underline(
                  hint: c == null || c.pattern.isEmpty
                      ? 'Telefon raqami'
                      : c.pattern.replaceAll('X', '0'),
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 26),
        Center(
          child: TextButton.icon(
            onPressed: _busy || !_available ? null : _startQr,
            icon: const Icon(Icons.qr_code_2_rounded),
            label: const Text('QR kod orqali kirish'),
            style: TextButton.styleFrom(
              foregroundColor: _Tg.accent,
              textStyle:
                  const TextStyle(fontSize: 15, fontWeight: FontWeight.w500),
            ),
          ),
        ),
      ],
    );
  }

  Future<void> _pickCountry() async {
    final picked = await Navigator.of(context).push<TgCountry>(
      MaterialPageRoute(builder: (_) => const _CountryPicker()),
    );
    if (picked == null || !mounted) return;
    _ccTouched = true;
    setState(() {
      _country = picked;
      _cc.text = picked.code;
      _num.text = picked.format(_num.text.replaceAll(RegExp(r'[^0-9]'), ''));
    });
    _syncPhone();
    _focus.requestFocus();
  }

  Widget _codeBody() {
    final n = _codeLength;
    final text = _code.text;
    return Column(
      children: [
        _header(Icons.sms_outlined, _prettyPhone, _whereSent(), center: true),
        _Shake(
          trigger: _shake,
          child: GestureDetector(
            onTap: () => _focus.requestFocus(),
            child: Stack(
              alignment: Alignment.center,
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    for (var i = 0; i < n; i++)
                      AnimatedContainer(
                        duration: const Duration(milliseconds: 150),
                        margin: const EdgeInsets.symmetric(horizontal: 4),
                        width: 44,
                        height: 52,
                        alignment: Alignment.center,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(
                            width: 1.6,
                            color: _error != null
                                ? _Tg.red
                                : i == text.length ||
                                        (i == n - 1 && text.length == n)
                                    ? _Tg.accent
                                    : _Tg.line,
                          ),
                        ),
                        child: AnimatedSwitcher(
                          duration: const Duration(milliseconds: 150),
                          transitionBuilder: (c, a) =>
                              ScaleTransition(scale: a, child: c),
                          child: Text(
                            i < text.length ? text[i] : '',
                            key: ValueKey('$i${i < text.length ? text[i] : ''}'),
                            style: const TextStyle(
                                color: Colors.white,
                                fontSize: 22,
                                fontWeight: FontWeight.w500),
                          ),
                        ),
                      ),
                  ],
                ),
                // Ko'rinmas haqiqiy maydon — klaviatura va qo'yish
                // (paste) shu orqali ishlaydi.
                Positioned.fill(
                  child: Opacity(
                    opacity: 0,
                    child: TextField(
                      controller: _code,
                      focusNode: _focus,
                      autofocus: true,
                      enabled: !_busy,
                      showCursor: false,
                      keyboardType: TextInputType.number,
                      autofillHints: const [AutofillHints.oneTimeCode],
                      inputFormatters: [
                        FilteringTextInputFormatter.digitsOnly,
                        LengthLimitingTextInputFormatter(n),
                      ],
                      onChanged: (v) {
                        setState(() => _error = null);
                        if (v.length == n) _submit();
                      },
                      onSubmitted: (_) => _submit(),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: 28),
        _link(
          _resendLeft > 0
              ? '$_nextLabel (${_clock(_resendLeft)})'
              : _nextLabel,
          _busy || _resendLeft > 0 ? null : _resend,
        ),
        _link('Raqamni o\'zgartirish', _busy ? null : _backToPhone),
      ],
    );
  }

  Widget _passwordBody() {
    return Column(
      children: [
        _header(
            Icons.lock_outline_rounded,
            'Parolingiz',
            _hint.isEmpty
                ? 'Hisobingizda ikki bosqichli tekshiruv yoqilgan. '
                    'Parolingizni kiriting.'
                : 'Hisobingizda ikki bosqichli tekshiruv yoqilgan. '
                    'Parolingizni kiriting.\nEslatma: $_hint',
            center: true),
        _Shake(
          trigger: _shake,
          child: TextField(
            controller: _password,
            focusNode: _focus,
            autofocus: true,
            enabled: !_busy,
            obscureText: !_showPassword,
            keyboardType: TextInputType.visiblePassword,
            style: const TextStyle(color: Colors.white, fontSize: 18),
            cursorColor: _Tg.accent,
            onSubmitted: (_) => _submit(),
            decoration: _underline(
              hint: 'Parol',
              suffix: IconButton(
                icon: Icon(
                  _showPassword
                      ? Icons.visibility_off_outlined
                      : Icons.visibility_outlined,
                  color: _Tg.hint,
                ),
                onPressed: () =>
                    setState(() => _showPassword = !_showPassword),
              ),
            ),
          ),
        ),
        const SizedBox(height: 18),
        _link('Raqamni o\'zgartirish', _busy ? null : _backToPhone),
      ],
    );
  }

  Widget _qrBody() {
    const steps = [
      'Telefoningizda Telegram\'ni oching',
      'Sozlamalar → Qurilmalar → Qurilmani ulash',
      'Kirishni tasdiqlash uchun telefonni shu QR kodga qarating',
    ];
    return Column(
      children: [
        const SizedBox(height: 8),
        Center(
          child: Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              color: Colors.white,
              borderRadius: BorderRadius.circular(20),
            ),
            child: SizedBox(
              width: 220,
              height: 220,
              child: AnimatedSwitcher(
                duration: const Duration(milliseconds: 250),
                child: _qrUrl.isEmpty
                    ? const Center(
                        child: CircularProgressIndicator(color: _Tg.accent),
                      )
                    : QrImageView(
                        key: ValueKey(_qrUrl),
                        data: _qrUrl,
                        size: 220,
                        backgroundColor: Colors.white,
                        padding: EdgeInsets.zero,
                        eyeStyle: const QrEyeStyle(
                            eyeShape: QrEyeShape.circle,
                            color: Color(0xFF1C2733)),
                        dataModuleStyle: const QrDataModuleStyle(
                            dataModuleShape: QrDataModuleShape.circle,
                            color: Color(0xFF1C2733)),
                      ),
              ),
            ),
          ),
        ),
        const SizedBox(height: 28),
        const Text('QR kod orqali Telegram\'ga kirish',
            textAlign: TextAlign.center,
            style: TextStyle(
                color: Colors.white, fontSize: 21, fontWeight: FontWeight.w600)),
        const SizedBox(height: 20),
        for (var i = 0; i < steps.length; i++)
          Padding(
            padding: const EdgeInsets.only(bottom: 14),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Container(
                  width: 24,
                  height: 24,
                  alignment: Alignment.center,
                  decoration: const BoxDecoration(
                      color: _Tg.accent, shape: BoxShape.circle),
                  child: Text('${i + 1}',
                      style: const TextStyle(
                          color: Colors.white,
                          fontSize: 13,
                          fontWeight: FontWeight.w600)),
                ),
                const SizedBox(width: 14),
                Expanded(
                  child: Text(steps[i],
                      style: const TextStyle(
                          color: Colors.white, fontSize: 15, height: 1.4)),
                ),
              ],
            ),
          ),
        const SizedBox(height: 6),
        _link('Raqam orqali kirish', _busy
            ? null
            : () {
                _stopQr();
                _go(_Step.phone);
              }),
      ],
    );
  }

  Widget _stepBody() {
    switch (_step) {
      case _Step.phone:
        return _phoneBody();
      case _Step.qr:
        return _qrBody();
      case _Step.code:
        return _codeBody();
      case _Step.password:
        return _passwordBody();
      case _Step.finishing:
        return Column(
          children: [
            _header(Icons.verified_user_outlined, 'Kirilmoqda',
                'Hisobingiz tasdiqlanmoqda...',
                center: true),
            if (_error == null)
              const Center(
                child: CircularProgressIndicator(color: _Tg.accent),
              ),
          ],
        );
    }
  }
}

/// Telegram Android (tungi) ranglari.
abstract final class _Tg {
  static const bg = Color(0xFF1C242F);
  static const accent = Color(0xFF50A8EB);
  static const accentDark = Color(0xFF3A8FD6);
  static const hint = Color(0xFF7D8B99);
  static const faint = Color(0xFF4F5D6B);
  static const line = Color(0xFF34414E);
  static const red = Color(0xFFE5575F);
}

/// Raqam maydoni: faqat raqamlar, davlat shabloni bo'yicha
/// bo'laklanadi (`90 123 45 67`).
class _NumberFormatter extends TextInputFormatter {
  final TgCountry? Function() country;
  _NumberFormatter(this.country);

  @override
  TextEditingValue formatEditUpdate(
      TextEditingValue oldValue, TextEditingValue newValue) {
    var digits = newValue.text.replaceAll(RegExp(r'[^0-9]'), '');
    final c = country();
    final max = c == null || c.length == 0 ? 15 : c.length;
    if (digits.length > max) digits = digits.substring(0, max);
    final text = c == null ? digits : c.format(digits);
    return TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );
  }
}

/// Xato bo'lganda chayqaladi (Telegram'dagi kabi). [trigger]
/// o'zgarganda bir marta o'ynaydi.
class _Shake extends StatelessWidget {
  final int trigger;
  final Widget child;
  const _Shake({required this.trigger, required this.child});

  @override
  Widget build(BuildContext context) {
    if (trigger == 0) return child;
    return TweenAnimationBuilder<double>(
      key: ValueKey(trigger),
      tween: Tween(begin: 0, end: 1),
      duration: const Duration(milliseconds: 420),
      builder: (context, t, c) => Transform.translate(
        offset: Offset(math.sin(t * math.pi * 6) * 10 * (1 - t), 0),
        child: c,
      ),
      child: child,
    );
  }
}

/// Davlat tanlash sahifasi (qidiruv bilan).
class _CountryPicker extends StatefulWidget {
  const _CountryPicker();

  @override
  State<_CountryPicker> createState() => _CountryPickerState();
}

class _CountryPickerState extends State<_CountryPicker> {
  String _q = '';

  @override
  Widget build(BuildContext context) {
    final q = _q.toLowerCase().replaceAll('+', '');
    final list = tgCountries
        .where((c) => q.isEmpty || c.name.toLowerCase().contains(q) ||
            c.code.startsWith(q))
        .toList();
    return Scaffold(
      backgroundColor: _Tg.bg,
      appBar: AppBar(
        backgroundColor: const Color(0xFF232E3C),
        iconTheme: const IconThemeData(color: Colors.white),
        title: TextField(
          autofocus: true,
          style: const TextStyle(color: Colors.white, fontSize: 18),
          cursorColor: _Tg.accent,
          decoration: const InputDecoration(
            hintText: 'Qidirish',
            hintStyle: TextStyle(color: _Tg.hint),
            border: InputBorder.none,
          ),
          onChanged: (v) => setState(() => _q = v.trim()),
        ),
      ),
      body: ListView.builder(
        itemCount: list.length,
        itemBuilder: (_, i) {
          final c = list[i];
          return ListTile(
            leading: Text(c.flag, style: const TextStyle(fontSize: 24)),
            title: Text(c.name,
                style: const TextStyle(color: Colors.white, fontSize: 16)),
            trailing: Text('+${c.code}',
                style: const TextStyle(color: _Tg.accent, fontSize: 16)),
            onTap: () => Navigator.of(context).pop(c),
          );
        },
      ),
    );
  }
}
