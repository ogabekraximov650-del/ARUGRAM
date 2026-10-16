// lib/services/api_base.dart — SERVER MANZILI (BITTA JOYDA)
//
// ARUMEDIATV worker'i `arumediatv` nomi bilan ishlaydi (eski `arumediatv` worker ustiga
// deploy qilingan — foydalanuvchi talabi: eski ilova endi ulana olmaydi).
//
// Manzil APK yig'ilayotganda `--dart-define=API_BASE=...` bilan
// beriladi: CI uni Cloudflare hisobining workers.dev subdomenidan
// o'zi hisoblaydi (`build-flutter-apk.yml`). Berilmasa — quyidagi
// zaxira qiymat.
const String kApiBase = String.fromEnvironment(
  'API_BASE',
  defaultValue: 'https://arumediatv.uzcom.workers.dev',
);
