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

class AuthGate extends StatelessWidget {
  final Widget child;
  const AuthGate({super.key, required this.child});

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
        if (auth.isLoggedIn && TelegramService.instance.authorized) {
          return child;
        }
        return const PhoneLoginScreen(gate: true);
      },
    );
  }
}
