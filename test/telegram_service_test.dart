import 'package:flutter_test/flutter_test.dart';
import 'package:soft/services/telegram_service.dart';

void main() {
  group('TelegramService.fileNameOf', () {
    // Rust'dagi kesh kaliti (`video_cache.rs` -> `cache_key`) bilan
    // bir xil bo'lishi SHART: Telegram'dan yuklangan bo'laklar B2
    // yo'lidagi bilan bitta papkaga tushadi.
    test('worker manzilidan fayl nomi', () {
      expect(
          TelegramService.fileNameOf(
              'https://arumediatv.uzcom.workers.dev/api/image/ep_12_720p_1700000000.mp4'),
          'ep_12_720p_1700000000.mp4');
    });

    test("so'rov qismi tashlanadi", () {
      expect(
          TelegramService.fileNameOf('https://x.dev/api/image/a-b.mp4?t=abc'),
          'a-b.mp4');
    });

    test('xavfli nomlar rad etiladi', () {
      expect(TelegramService.fileNameOf('https://x.dev/api/image/'), '');
      expect(TelegramService.fileNameOf('https://x.dev/api/image/..'), '');
      expect(TelegramService.fileNameOf('https://x.dev/a%20b.mp4'), '');
    });
  });
}
