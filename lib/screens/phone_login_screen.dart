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
//      `/start <token>` yuboradi (`rust_tg_start_bot`). Worker'dagi
//      bot orqali kirish tizimi o'zgarmagan: u xabarni kim
//      yuborganini Telegram'ning o'zidan biladi va sessiya ochadi.
//      Ilova esa kirish holatini so'rab turadi.
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

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../services/auth_service.dart';
import '../services/telegram_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../widgets/telegram_logo.dart';

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

/// Raqam maydoni: boshida DOIM `+`, keyin faqat raqamlar.
/// `+` ni o'chirib bo'lmaydi.
class PhoneNumberFormatter extends TextInputFormatter {
  @override
  TextEditingValue formatEditUpdate(
      TextEditingValue oldValue, TextEditingValue newValue) {
    final digits = newValue.text.replaceAll(RegExp(r'[^0-9]'), '');
    final cut = digits.length > 15 ? digits.substring(0, 15) : digits;
    final text = '+$cut';
    return TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );
  }
}

class _PhoneLoginScreenState extends State<PhoneLoginScreen> {
  final _phone = TextEditingController(text: '+');
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
    if (_busy || _resendLeft > 0) return;
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
    if (r.error != null) return _fail(r.error);
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
  }

  void _restoreStage() {
    final st = _tg.loginState();
    final phone = (st['phone'] as String?) ?? '';
    if (phone.isNotEmpty) _phone.text = phone;
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
    _qrTimer?.cancel();
    _resendTimer?.cancel();
    _phone.dispose();
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

  void _fail(String? e) {
    if (!mounted) return;
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
        if (r.error != null) return _fail(r.error);
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
        if (!r.done) return _fail(r.error);
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
          return _fail(r.error ?? 'Parol noto\'g\'ri');
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

  @override
  Widget build(BuildContext context) {
    return AppBackground(
      child: Scaffold(
        backgroundColor: Colors.transparent,
        appBar: AppBar(
          backgroundColor: Colors.transparent,
          elevation: 0,
          iconTheme: const IconThemeData(color: Colors.white),
          automaticallyImplyLeading: false,
          // Darvoza rejimida raqam bosqichida orqaga yo'l yo'q —
          // ilovaga faqat kirib o'tiladi.
          leading:
              (widget.gate && _step == _Step.phone) || _step == _Step.finishing
                  ? null
                  : IconButton(
                      icon: const Icon(Icons.arrow_back_rounded),
                      onPressed: () {
                        if (_step == _Step.qr) {
                          _stopQr();
                          _go(_Step.phone);
                        } else if (_step == _Step.code ||
                            _step == _Step.password) {
                          _backToPhone();
                        } else {
                          Navigator.of(context).maybePop();
                        }
                      },
                    ),
        ),
        floatingActionButton: _step == _Step.qr ||
                (_step == _Step.finishing && _error == null)
            ? null
            : FloatingActionButton(
                onPressed: _busy || !_available ? null : _submit,
                backgroundColor: AppColors.telegram,
                shape: const CircleBorder(),
                child: _busy
                    ? const SizedBox(
                        width: 22,
                        height: 22,
                        child: CircularProgressIndicator(
                            strokeWidth: 2.4, color: Colors.white),
                      )
                    : const Icon(Icons.arrow_forward_rounded,
                        color: Colors.white),
              ),
        body: SafeArea(
          top: false,
          child: ListView(
            padding: const EdgeInsets.fromLTRB(28, 12, 28, 120),
            children: [
              const Center(child: TelegramLogo(size: 92)),
              const SizedBox(height: 28),
              AnimatedSwitcher(
                duration: const Duration(milliseconds: 220),
                child: KeyedSubtree(
                  key: ValueKey(_step),
                  child: _stepBody(),
                ),
              ),
              if (_error != null) ...[
                const SizedBox(height: 16),
                Text(
                  _error!,
                  textAlign: TextAlign.center,
                  style:
                      const TextStyle(color: Colors.redAccent, fontSize: 13.5),
                ),
              ],
              if (!_available) ...[
                const SizedBox(height: 16),
                Text(
                  'Telegram orqali kirish hozircha yoqilmagan.',
                  textAlign: TextAlign.center,
                  style: TextStyle(
                      color: Colors.white.withValues(alpha: 0.6),
                      fontSize: 13.5),
                ),
              ],
              // "Bot orqali kirish" OLIB TASHLANDI (foydalanuvchi
              // talabi): bot orqali kirish endi faqat Telegram
              // hisobiga kirilgach, avtomatik (`_finish`).
              if (_step == _Step.code) ...[
                const SizedBox(height: 16),
                Center(
                  child: TextButton(
                    onPressed: _busy || _resendLeft > 0 ? null : _resend,
                    child: Text(
                      _resendLeft > 0
                          ? '$_nextLabel (${_resendLeft ~/ 60}:${(_resendLeft % 60).toString().padLeft(2, '0')})'
                          : _nextLabel,
                      style: TextStyle(
                        color: _resendLeft > 0
                            ? Colors.white38
                            : AppColors.telegramLight,
                      ),
                    ),
                  ),
                ),
              ],
              if (_step == _Step.code || _step == _Step.password) ...[
                const SizedBox(height: 8),
                Center(
                  child: TextButton(
                    onPressed: _busy ? null : _backToPhone,
                    child: const Text(
                      'Raqamni o\'zgartirish',
                      style: TextStyle(color: AppColors.telegramLight),
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

  Widget _title(String title, String subtitle) {
    return Column(
      children: [
        Text(
          title,
          textAlign: TextAlign.center,
          style: const TextStyle(
            color: Colors.white,
            fontSize: 22,
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: 10),
        Text(
          subtitle,
          textAlign: TextAlign.center,
          style: TextStyle(
            color: Colors.white.withValues(alpha: 0.6),
            fontSize: 14,
            height: 1.45,
          ),
        ),
        const SizedBox(height: 28),
      ],
    );
  }

  InputDecoration _decoration(String label, {Widget? suffix}) {
    return InputDecoration(
      labelText: label,
      labelStyle: const TextStyle(color: Colors.white54),
      floatingLabelStyle: const TextStyle(color: AppColors.telegramLight),
      suffixIcon: suffix,
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: const BorderSide(color: Colors.white24),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: const BorderSide(color: AppColors.telegram, width: 1.6),
      ),
      disabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: const BorderSide(color: Colors.white12),
      ),
    );
  }

  Widget _stepBody() {
    const fieldStyle = TextStyle(
      color: Colors.white,
      fontSize: 20,
      letterSpacing: 1.2,
    );
    switch (_step) {
      case _Step.phone:
        return Column(
          children: [
            _title('Telefon raqamingiz',
                'Telegram hisobingiz ulangan raqamni xalqaro formatda kiriting.'),
            TextField(
              controller: _phone,
              focusNode: _focus,
              autofocus: true,
              enabled: !_busy && _available,
              keyboardType: TextInputType.phone,
              inputFormatters: [PhoneNumberFormatter()],
              style: fieldStyle,
              onSubmitted: (_) => _submit(),
              decoration: _decoration('Telefon raqami'),
            ),
            const SizedBox(height: 18),
            TextButton.icon(
              onPressed: _busy || !_available ? null : _startQr,
              icon: const Icon(Icons.qr_code_2_rounded,
                  color: AppColors.telegramLight),
              label: const Text(
                'QR kod orqali kirish',
                style: TextStyle(color: AppColors.telegramLight),
              ),
            ),
          ],
        );
      case _Step.qr:
        return Column(
          children: [
            _title(
                'QR orqali kirish',
                'Boshqa qurilmadagi Telegram\'ni oching: Sozlamalar → '
                    'Qurilmalar → "Qurilmani ulash" va shu QR\'ni skanerlang. '
                    'Kod kerak emas.'),
            const SizedBox(height: 8),
            Center(
              child: Container(
                padding: const EdgeInsets.all(14),
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.circular(18),
                ),
                child: SizedBox(
                  width: 220,
                  height: 220,
                  child: _qrUrl.isEmpty
                      ? const Center(
                          child: CircularProgressIndicator(
                              color: AppColors.telegram),
                        )
                      : QrImageView(
                          data: _qrUrl,
                          size: 220,
                          backgroundColor: Colors.white,
                          padding: EdgeInsets.zero,
                        ),
                ),
              ),
            ),
          ],
        );
      case _Step.code:
        return Column(
          children: [
            _title('Kodni kiriting', _whereSent()),
            TextField(
              controller: _code,
              focusNode: _focus,
              autofocus: true,
              enabled: !_busy,
              keyboardType: TextInputType.number,
              inputFormatters: [
                FilteringTextInputFormatter.digitsOnly,
                LengthLimitingTextInputFormatter(8),
              ],
              textAlign: TextAlign.center,
              style: fieldStyle.copyWith(fontSize: 26, letterSpacing: 10),
              // Kod uzunligini Telegram aytadi (odatda 5) — to'lishi
              // bilan o'zi yuboriladi.
              onChanged: (v) {
                if (v.length == _codeLength) _submit();
              },
              onSubmitted: (_) => _submit(),
              decoration: _decoration('Kod'),
            ),
          ],
        );
      case _Step.password:
        return Column(
          children: [
            _title(
                'Ikki bosqichli parol',
                _hint.isEmpty
                    ? 'Hisobingizda qo\'shimcha parol yoqilgan.'
                    : 'Hisobingizda qo\'shimcha parol yoqilgan.\nEslatma: $_hint'),
            TextField(
              controller: _password,
              focusNode: _focus,
              autofocus: true,
              enabled: !_busy,
              obscureText: !_showPassword,
              keyboardType: TextInputType.visiblePassword,
              style: fieldStyle.copyWith(letterSpacing: 0.5),
              onSubmitted: (_) => _submit(),
              decoration: _decoration(
                'Parol',
                suffix: IconButton(
                  icon: Icon(
                    _showPassword
                        ? Icons.visibility_off_rounded
                        : Icons.visibility_rounded,
                    color: Colors.white54,
                  ),
                  onPressed: () =>
                      setState(() => _showPassword = !_showPassword),
                ),
              ),
            ),
          ],
        );
      case _Step.finishing:
        return Column(
          children: [
            _title('Kirilmoqda', 'Hisobingiz tasdiqlanmoqda...'),
            if (_error == null)
              const Center(
                child: CircularProgressIndicator(color: AppColors.telegram),
              ),
          ],
        );
    }
  }
}
