import 'package:flutter_test/flutter_test.dart';
import 'package:soft/widgets/emoji_text.dart';
import 'package:soft/widgets/tg_composer.dart';

void main() {
  test('matn o\'zgarishsiz yuboriladi', () {
    final c = TgTextController();
    c.insertText('Salom ');
    c.insertText('😀!');
    expect(c.encoded, 'Salom 😀!');
  });

  test('⌫ emoji o\'rtasidan kesmaydi', () {
    final c = TgTextController();
    c.insertText('ok👨‍👩‍👧');
    c.backspace();
    expect(c.text, 'ok');
  });

  test('eski maxsus emoji belgilari oddiy emojiga aylanadi', () {
    expect(plainEmojiText('a [ce:12:😀] b [ce:-7:🔥]'), 'a 😀 b 🔥');
    expect(plainEmojiText('oddiy matn'), 'oddiy matn');
  });
}
