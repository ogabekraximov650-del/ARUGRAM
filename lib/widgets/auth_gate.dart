// lib/widgets/auth_gate.dart — TO'LIQ KIRMAGUNCHA ILOVA YOPIQ
//
// TALAB (foydalanuvchi): "Telegram accountiga kirmagan foydalanuvchida
// ilova ishlamasligi kerak — faqat accountga to'liq kirganda
// ishlashi kerak".
//
// "To'liq kirgan" = ilova hisobi (worker sessiyasi) BOR va Telegram
// hisobi ulangan. Ikkalasidan biri yo'q bo'lsa butun ilova o'rnida
// Telegram'dagidek kirish oynasi turadi (`PhoneLoginScreen`, darvoza
// rejimi). Kirilishi bilan ilova o'zi ochiladi; hisobdan chiqilsa
// yana shu oyna.

import 'package:flutter/material.dart';

import '../screens/phone_login_screen.dart';
import '../services/auth_service.dart';
import '../services/telegram_service.dart';

class AuthGate extends StatefulWidget {
  final Widget child;
  const AuthGate({super.key, required this.child});

  @override
  State<AuthGate> createState() => _AuthGateState();
}

class _AuthGateState extends State<AuthGate> {
  /// Oxirgi chizishda ilova ochiq edimi.
  bool _wasOpen = false;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation:
          Listenable.merge([AuthService.instance, TelegramService.instance]),
      builder: (context, _) {
        final auth = AuthService.instance;
        // Saqlangan hisob hali o'qilmagan — kirish oynasi bir zumga
        // miltillab ketmasin.
        if (!auth.restored) return const SizedBox.shrink();
        final open = auth.isLoggedIn && TelegramService.instance.authorized;
        if (_wasOpen && !open) {
          // ── SESSIYA UZILDI ──────────────────────────────────
          // Foydalanuvchi talabi: Telegram sessiyasi uzilishi bilan
          // raqam oynasi chiqsin. Darvoza ilovaning ENG PASTKI
          // sahifasi — ustida ochiq pleyer (yoki boshqa sahifa)
          // bo'lsa, oyna uning ortida qolib ketardi. Shu sabab
          // ochiq sahifalar yopiladi.
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (!mounted) return;
            Navigator.of(context).popUntil((r) => r.isFirst);
          });
        }
        _wasOpen = open;
        if (open) return widget.child;
        return const PhoneLoginScreen(gate: true);
      },
    );
  }
}
