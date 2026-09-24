// lib/services/api_base.dart — SERVER MANZILI (BITTA JOYDA)
//
// ARUGRAM o'zining ALOHIDA worker'ida (`arugram`) ishlaydi — eski
// `arumediatv` ga tegilmaydi (foydalanuvchi talabi).
//
// Manzil APK yig'ilayotganda `--dart-define=API_BASE=...` bilan
// beriladi: CI uni Cloudflare hisobining workers.dev subdomenidan
// o'zi hisoblaydi (`build-flutter-apk.yml`). Berilmasa — quyidagi
// zaxira qiymat.
const String kApiBase = String.fromEnvironment(
  'API_BASE',
  defaultValue: 'https://arugram.uzcom.workers.dev',
);
