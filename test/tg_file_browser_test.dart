import 'package:flutter_test/flutter_test.dart';
import 'package:soft/widgets/tg_file_browser.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('kengaytma va ruxsat etilgan fayllar', () {
    expect(TgFiles.ext('A.MP4'), 'mp4');
    expect(TgFiles.ext('nomsiz'), '');
    expect(TgFiles.allowed('x.png', kMediaExts), isTrue);
    expect(TgFiles.allowed('x.apk', kMediaExts), isFalse);
    expect(TgFiles.allowed('x.apk', null), isTrue);
  });

  test('hajm matni', () {
    expect(TgFiles.size(512), '512 B');
    expect(TgFiles.size(1536), '1.5 KB');
    expect(TgFiles.size(5 * 1048576), '5.0 MB');
  });

  test('ruxsat kanali yo\'q bo\'lsa — bor deb olinadi', () async {
    expect(await TgFiles.hasAccess(), isTrue);
  });
}
