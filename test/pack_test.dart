// test/pack_test.dart — EMOJI, GIF VA STIKER TO'PLAMLARI.
//
// Sof qoidalar: havola va belgi formati, sarlavha o'qish, fayl turini
// aniqlash, yozish paneli va navbat. Tarmoq ham, ekran ham kerak emas.

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:soft/services/pack_service.dart';
import 'package:soft/services/sync_queue.dart';
import 'package:soft/widgets/emoji_text.dart';
import 'package:soft/widgets/tg_composer.dart';

void main() {
  group('havola va belgilar', () {
    test('pk_<to\'plam>_<element> tahlili', () {
      expect(parsePackRef('pk_123_7'), (123, 7));
      expect(parsePackRef('pk_123_'), isNull);
      expect(parsePackRef('pk__7'), isNull);
      expect(parsePackRef('pk_12a_7'), isNull);
      expect(parsePackRef('pk_0_7'), isNull);
      expect(parsePackRef('pk_5_0'), isNull);
      expect(parsePackRef('xx_5_1'), isNull);
      expect(parsePackRef('pk_5_1_2'), isNull);
    });

    test('maxsus emoji belgisi', () {
      expect(packEmojiToken(12, 3, '😀'), '[pe:12:3:😀]');
      // Belgi ichida qavs bo'lmasin.
      expect(packEmojiToken(1, 2, '[x]'), '[pe:1:2:x]');
      final m = kPackEmojiToken.firstMatch('salom [pe:12:3:😀] dunyo')!;
      expect(m.group(1), '12');
      expect(m.group(2), '3');
      expect(m.group(3), '😀');
    });

    test('ro\'yxatda maxsus emoji oddiy emoji bo\'lib chiqadi', () {
      expect(plainEmojiText('a [pe:12:3:😀] b'), 'a 😀 b');
      // Eski Telegram belgisi bilan aralashmaydi.
      expect(plainEmojiText('[ce:5:🔥] [pe:1:2:😎]'), '🔥 😎');
    });

    test('rasm turi baytlardan aniqlanadi', () {
      expect(sniffImage([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]), 'png');
      expect(sniffImage([0xFF, 0xD8, 0xFF, 0xE0]), 'jpeg');
      expect(sniffImage('GIF89a'.codeUnits), 'gif');
      expect(
          sniffImage([...'RIFF'.codeUnits, 0, 0, 0, 0, ...'WEBP'.codeUnits]),
          'webp');
      expect(sniffImage([0, 0, 0, 24, ...'ftypmp42'.codeUnits]), 'mp4');
      expect(sniffImage([0x1A, 0x45, 0xDF, 0xA3, 0, 0, 0, 0]), 'webm');
      expect(isPackVideo('mp4'), isTrue);
      expect(isPackVideo('webm'), isTrue);
      expect(isPackVideo('png'), isFalse);
      expect(sniffImage('MZ....'.codeUnits), '');
      expect(sniffImage(const []), '');
    });
  });

  group('sarlavha', () {
    test('Python (`arupack.py`) yozgan sarlavha o\'qiladi', () {
      const raw = '{"v":1,"id":77,"kind":"sticker","title":"Mushuklar",'
          '"ver":3,"next":3,"items":['
          '{"i":1,"o":8,"l":4,"to":0,"tl":2,"a":0,"w":512,"h":512,"e":"😀"},'
          '{"i":2,"o":12,"l":6,"to":2,"tl":2,"a":1,"w":256,"h":300,"e":"🐱"}]}';
      final h = PackHeader.fromJson(
          jsonDecode(raw) as Map<String, dynamic>, 16 + raw.length);
      expect(h.id, 77);
      expect(h.ver, 3);
      expect(h.items.length, 2);
      expect(h.find(2)!.animated, isTrue);
      expect(h.find(1)!.animated, isFalse);
      expect(h.find(9), isNull);
      // Kichik rasmlar bloki elementlardan OLDIN tugaydi.
      expect(h.thumbEnd, 4);
      expect(h.items.every((i) => i.off >= h.thumbEnd), isTrue);
    });

    test('keshga yozib qayta o\'qiladi', () {
      final h = PackHeader(
        id: 5,
        kind: 'gif',
        title: 'G',
        ver: 2,
        base: 100,
        items: const [
          PackItem(
              id: 1,
              off: 10,
              len: 20,
              thumbOff: 0,
              thumbLen: 5,
              animated: true,
              w: 1,
              h: 2,
              emoji: '')
        ],
      );
      final back = PackHeader.fromCache(h.toCache())!;
      expect(back.ver, 2);
      expect(back.base, 100);
      expect(back.find(1)!.len, 20);
      expect(PackHeader.fromCache({'base': 3}), isNull);
    });
  });

  group('yozish paneli', () {
    test('maxsus emoji maydonda BITTA belgi, yuborishda belgiga aylanadi', () {
      final c = TgTextController();
      c.insertText('a ');
      c.insertPackEmoji(const PackPick(PackKind.emoji, 12, 3, '😀'));
      c.insertText(' b');
      // Matnda kursor uchun bitta belgi.
      expect(c.text.length, 5);
      expect(c.encoded, 'a [pe:12:3:😀] b');
      expect(c.isBlank, isFalse);
    });

    test('⌫ maxsus emojini bitta bosishda o\'chiradi', () {
      final c = TgTextController();
      c.insertPackEmoji(const PackPick(PackKind.emoji, 1, 2, '😀'));
      c.backspace();
      expect(c.text, '');
      expect(c.encoded, '');
    });

    test('bir xil element bir xil belgi oladi; tozalanganda belgi qolmaydi', () {
      final c = TgTextController();
      c.insertPackEmoji(const PackPick(PackKind.emoji, 1, 2, '😀'));
      c.insertPackEmoji(const PackPick(PackKind.emoji, 1, 2, '😀'));
      expect(c.text.length, 2);
      expect(c.text.codeUnitAt(0), c.text.codeUnitAt(1));
      c.clear();
      c.insertText('oddiy');
      expect(c.encoded, 'oddiy');
    });
  });

  group('saralanganlar', () {
    test('qo\'shiladi va olib tashlanadi', () {
      final svc = PackService.instance;
      const p = PackPick(PackKind.gif, 7, 3, '😂');
      expect(svc.isFavorite(p), isFalse);
      expect(svc.toggleFavorite(p), isTrue);
      expect(svc.isFavorite(p), isTrue);
      expect(svc.favorites(PackKind.gif).first.item, 3);
      expect(svc.toggleFavorite(p), isFalse);
      expect(svc.isFavorite(p), isFalse);
    });
  });

  group('navbat', () {
    final q = SyncQueue.instance;
    setUp(() => q.wipe());

    test('to\'plam amallari navbatda tartib bilan turadi', () {
      q.putPack('p:new:5', {'op': 'new', 'id': 5, 'kind': 'gif', 'title': 'G'});
      q.putPack('p:add:pki_1_a', {'op': 'add', 'pack': 5, 'file': 'pki_1_a'});
      expect(q.pendingPacks().map((m) => m['op']).toList(), ['new', 'add']);
    });

    test('obuna yoqilib o\'chirilsa navbatda bitta qator qoladi', () {
      q.putPack('p:sub:9', {'op': 'sub', 'pack': 9, 'on': true});
      q.putPack('p:sub:9', {'op': 'sub', 'pack': 9, 'on': false});
      final l = q.pendingPacks();
      expect(l.length, 1);
      expect(l.first['on'], isFalse);
    });
  });
}
