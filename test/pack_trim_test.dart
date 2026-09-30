import 'package:flutter_test/flutter_test.dart';
import 'package:soft/screens/pack_video_trim_screen.dart';
import 'package:soft/services/pack_service.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('eng uzun bo\'lak tur bo\'yicha belgilanadi', () {
    expect(packMaxSeconds(PackKind.emoji), 5);
    expect(packMaxSeconds(PackKind.sticker), 8);
    expect(packMaxSeconds(PackKind.gif), 15);
  });

  test('kadr kanali yo\'q bo\'lsa bo\'sh ro\'yxat (yiqilmaydi)', () async {
    expect(await grabVideoFrames('/yoq.mp4'), isEmpty);
  });
}
