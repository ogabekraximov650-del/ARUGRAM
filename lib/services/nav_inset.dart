// lib/services/nav_inset.dart — TIZIM TUGMALARI BALANDLIGI (ZAXIRA)
//
// TOPILGAN XATO (foydalanuvchi: "ekrandagi ba'zi narsalar to'liq
// ko'rinmayapti" — baholash oynasi va yuklab olish oynasining pastki
// tugmalari telefon tugmalari ortida qolardi): ba'zi telefonlarda
// (MIUI, ayniqsa pleyer to'liq ekrandan qaytgach) Flutter'ga pastki
// chekinish 0 bo'lib keladi, ilova esa tugmalar ostigacha chiziladi.
// Natijada `SafeArea` va `viewPadding` ga tayangan hamma oynalar
// tugmalar ortiga kirib ketardi.
//
// Yechim: tizim tugmalarining "barqaror" balandligi Android'ning
// o'zidan so'raladi (`aru/insets` → `navBottom`, yashirilgan paytda
// ham o'zgarmaydi) va ilova ildizida `MediaQuery` ning pastki
// chekinishi KAMIDA shuncha qilib qo'yiladi (`main.dart` → `builder`).
// Pleyer to'liq ekranda (tugmalar yashirin) va klaviatura ochiq
// paytda qo'llanmaydi.

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

class NavInset {
  NavInset._();

  static const _ch = MethodChannel('aru/insets');

  /// Tizim tugmalari balandligi, FIZIK pikselda (noma'lum — 0).
  static final ValueNotifier<double> px = ValueNotifier(0);

  /// Pleyer to'liq ekranda — tizim tugmalari yashirin, zaxira kerak emas.
  static final ValueNotifier<bool> immersive = ValueNotifier(false);

  /// Android'dan qayta o'qiydi (ishga tushganda, ilovaga qaytganda va
  /// to'liq ekrandan chiqqanda).
  static Future<void> refresh() async {
    if (defaultTargetPlatform != TargetPlatform.android) return;
    try {
      final v = await _ch.invokeMethod<int>('navBottom');
      if (v != null && v >= 0) px.value = v.toDouble();
    } catch (_) {}
  }
}
