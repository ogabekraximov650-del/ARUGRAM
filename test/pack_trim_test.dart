import 'package:flutter_test/flutter_test.dart';
import 'package:soft/screens/pack_video_trim_screen.dart';
import 'package:soft/services/pack_service.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('eng uzun bo\'lak tur bo\'yicha belgilanadi', () {
    expect(packMaxSeconds(PackKind.emoji), 8);
    expect(packMaxSeconds(PackKind.sticker), 12);
    expect(packMaxSeconds(PackKind.gif), 0); // cheklanmaydi, faqat 5 MB
  });

  test('kadr kanali yo\'q bo\'lsa bo\'sh ro\'yxat (yiqilmaydi)', () async {
    expect(await grabVideoFrames('/yoq.mp4'), isEmpty);
  });
}
