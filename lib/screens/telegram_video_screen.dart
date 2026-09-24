// lib/screens/telegram_video_screen.dart — TELEGRAM ORQALI KO'RISH
//
// Foydalanuvchi shu yerda o'z Telegram hisobini ilovaga ulaydi:
// telefon raqami -> Telegram yuborgan kod -> (yoqilgan bo'lsa)
// 2 bosqichli parol. Ulangach videolar Telegram serveridan olinadi
// (`telegram_service.dart` izohiga qarang).
//
// Bu ilovaga KIRISH emas: ilova hisobi hozirgidek bot orqali. Bu
// faqat videolarni Telegram'dan olish uchun alohida ulanish.

import 'package:flutter/material.dart';

import '../services/telegram_service.dart';
import '../theme/app_background.dart';
import '../widgets/glass.dart';
import '../widgets/telegram_logo.dart';

enum _Step { phone, code, password }

class TelegramVideoScreen extends StatefulWidget {
  const TelegramVideoScreen({super.key});

  @override
  State<TelegramVideoScreen> createState() => _TelegramVideoScreenState();
}

class _TelegramVideoScreenState extends State<TelegramVideoScreen> {
  final _phone = TextEditingController();
  final _code = TextEditingController();
  final _password = TextEditingController();
  _Step _step = _Step.phone;
  bool _busy = false;
  String? _error;
  String _hint = '';

  TelegramService get _tg => TelegramService.instance;

  @override
  void initState() {
    super.initState();
    _tg.refreshConfig();
  }

  @override
  void dispose() {
    _phone.dispose();
    _code.dispose();
    _password.dispose();
    super.dispose();
  }

  Future<void> _run(Future<TgLoginStep> Function() call) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final r = await call();
    if (!mounted) return;
    setState(() {
      _busy = false;
      _error = r.error;
      if (r.done) {
        _step = _Step.phone;
        _code.clear();
        _password.clear();
      } else if (r.needPassword) {
        _hint = r.hint;
        _step = _Step.password;
      }
    });
  }

  Future<void> _sendPhone() => _run(() async {
        var p = _phone.text.replaceAll(RegExp(r'[\s()-]'), '');
        if (p.isNotEmpty && !p.startsWith('+')) p = '+$p';
        final r = await _tg.requestCode(p);
        if (r.error == null && mounted) setState(() => _step = _Step.code);
        return r;
      });

  Future<void> _sendCode() => _run(() => _tg.signIn(_code.text.trim()));

  Future<void> _sendPassword() => _run(() => _tg.checkPassword(_password.text));

  Future<void> _logout() async {
    setState(() => _busy = true);
    await _tg.logout();
    if (mounted) setState(() => _busy = false);
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
          title: const Text('Telegram orqali ko\'rish',
              style: TextStyle(color: Colors.white, fontSize: 18)),
        ),
        body: SafeArea(
          top: false,
          child: AnimatedBuilder(
            animation: _tg,
            builder: (context, _) => ListView(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 32),
              children: [
                const Center(child: TelegramLogo(size: 72)),
                const SizedBox(height: 16),
                Text(
                  'Telegram hisobingizni ulasangiz, videolar Telegram '
                  'serveridan olinadi. Ilovaga kirish o\'zgarmaydi — '
                  'bu faqat videolar uchun.',
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    color: Colors.white.withValues(alpha: 0.6),
                    fontSize: 13,
                    height: 1.5,
                  ),
                ),
                const SizedBox(height: 20),
                if (_tg.authorized) _connected() else _form(),
                if (_error != null) ...[
                  const SizedBox(height: 14),
                  Text(_error!,
                      textAlign: TextAlign.center,
                      style: const TextStyle(
                          color: Colors.redAccent, fontSize: 13)),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _connected() {
    return Glass(
      borderRadius: 18,
      padding: const EdgeInsets.all(16),
      child: Column(
        children: [
          const Icon(Icons.check_circle_rounded,
              color: Colors.greenAccent, size: 40),
          const SizedBox(height: 8),
          const Text('Telegram ulangan',
              style: TextStyle(
                  color: Colors.white,
                  fontSize: 16,
                  fontWeight: FontWeight.w700)),
          const SizedBox(height: 6),
          Text(
            'Kanalga yuklangan qismlar Telegram\'dan ko\'rsatiladi, '
            'qolganlari odatdagidek.',
            textAlign: TextAlign.center,
            style: TextStyle(
                color: Colors.white.withValues(alpha: 0.55), fontSize: 12.5),
          ),
          const SizedBox(height: 14),
          OutlinedButton(
            onPressed: _busy ? null : _logout,
            style: OutlinedButton.styleFrom(
              foregroundColor: Colors.white,
              side: const BorderSide(color: Colors.white24),
            ),
            child: const Text('Telegram\'dan uzish'),
          ),
        ],
      ),
    );
  }

  Widget _form() {
    late final String label;
    late final TextEditingController ctrl;
    late final VoidCallback submit;
    var keyboard = TextInputType.number;
    var obscure = false;
    switch (_step) {
      case _Step.phone:
        label = 'Telefon raqami (+998...)';
        ctrl = _phone;
        submit = _sendPhone;
        keyboard = TextInputType.phone;
      case _Step.code:
        label = 'Telegram yuborgan kod';
        ctrl = _code;
        submit = _sendCode;
      case _Step.password:
        label = _hint.isEmpty
            ? 'Ikki bosqichli parol'
            : 'Ikki bosqichli parol (eslatma: $_hint)';
        ctrl = _password;
        submit = _sendPassword;
        keyboard = TextInputType.visiblePassword;
        obscure = true;
    }
    return Glass(
      borderRadius: 18,
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          TextField(
            key: ValueKey(_step),
            controller: ctrl,
            keyboardType: keyboard,
            obscureText: obscure,
            autofocus: true,
            enabled: !_busy,
            onSubmitted: (_) => submit(),
            style: const TextStyle(color: Colors.white),
            decoration: InputDecoration(
              labelText: label,
              labelStyle: const TextStyle(color: Colors.white54),
              enabledBorder: const UnderlineInputBorder(
                  borderSide: BorderSide(color: Colors.white24)),
              focusedBorder: const UnderlineInputBorder(
                  borderSide: BorderSide(color: AppColors.accent)),
            ),
          ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: _busy ? null : submit,
            style: FilledButton.styleFrom(backgroundColor: AppColors.accent),
            child: _busy
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(
                        strokeWidth: 2, color: Colors.white))
                : Text(_step == _Step.phone ? 'Kod olish' : 'Ulash'),
          ),
          if (_step != _Step.phone)
            TextButton(
              onPressed: _busy
                  ? null
                  : () => setState(() {
                        _step = _Step.phone;
                        _error = null;
                      }),
              child: const Text('Raqamni o\'zgartirish',
                  style: TextStyle(color: Colors.white54)),
            ),
        ],
      ),
    );
  }
}
