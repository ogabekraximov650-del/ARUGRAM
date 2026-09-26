import 'package:flutter_test/flutter_test.dart';
import 'package:soft/services/tg_media.dart';
import 'package:soft/widgets/tg_composer.dart';
import 'package:soft/widgets/tg_media_view.dart';

void main() {
  const star = TgDoc(id: '5368324170671202286', kind: 'tgs', emoji: '⭐', custom: true);

  test('maxsus emoji yuborishda belgiga aylanadi', () {
    final c = TgTextController();
    c.insertText('Salom ');
    c.insertCustom(star);
    c.insertText('!');
    expect(c.encoded, 'Salom [ce:5368324170671202286:⭐]!');
    expect(plainEmojiText(c.encoded), 'Salom ⭐!');
  });

  test('⌫ emoji o\'rtasidan kesmaydi', () {
    final c = TgTextController();
    c.insertText('ok👨‍👩‍👧');
    c.backspace();
    expect(c.text, 'ok');
    c.insertCustom(star);
    c.backspace();
    expect(c.text, 'ok');
  });

  test('stiker havolasi u64 hex ko\'rinishda', () {
    const d = TgDoc(id: '-2', kind: 'webp', setId: '255', setHash: '-1');
    expect(d.ref, 'stk_ff_ffffffffffffffff_fffffffffffffffe');
  });

  test('matndagi belgilar topiladi', () {
    final m = customEmojiToken.allMatches('a [ce:12:😀] b [ce:-7:🔥]').toList();
    expect(m.map((x) => x.group(1)), ['12', '-7']);
    expect(m.map((x) => x.group(2)), ['😀', '🔥']);
  });
}
