// test/pack_widget_test.dart — MAXSUS EMOJI EKRANDA.
//
// To'plam yuklanmasa (tarmoq yo'q) xato chiqmaydi va o'rnida oddiy emoji
// ko'rinadi. Yozish maydonida maxsus emoji BITTA belgi bo'lib turadi
// (kursor mos keladi).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:soft/services/pack_service.dart';
import 'package:soft/widgets/emoji_text.dart';
import 'package:soft/widgets/pack_views.dart';
import 'package:soft/widgets/tg_composer.dart';

Future<void> settle(WidgetTester t) async {
  await t.pump();
  // `infoFor` 40 ms kutib, so'ng tarmoqqa chiqadi (testda tarmoq yo'q).
  await t.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 400)));
  await t.pump(const Duration(milliseconds: 100));
}

void main() {
  testWidgets('xabar matnida maxsus emoji yuklanmasa oddiy emoji chiqadi',
      (t) async {
    await t.pumpWidget(const MaterialApp(
      home: Scaffold(
        body: EmojiText('salom [pe:12:3:😀] dunyo',
            style: TextStyle(color: Colors.white, fontSize: 16)),
      ),
    ));
    await settle(t);
    expect(t.takeException(), isNull);
    expect(find.byType(PackEmojiInline), findsOneWidget);
    expect(find.text('😀'), findsOneWidget);
  });

  testWidgets('belgisiz matn oddiy Text bo\'lib qoladi', (t) async {
    await t.pumpWidget(const MaterialApp(
      home: Scaffold(
        body: EmojiText('oddiy matn',
            style: TextStyle(color: Colors.white, fontSize: 16)),
      ),
    ));
    expect(find.byType(PackEmojiInline), findsNothing);
    expect(find.text('oddiy matn'), findsOneWidget);
  });

  testWidgets('yozish maydonida maxsus emoji chiziladi va kursor mos keladi',
      (t) async {
    final c = TgTextController();
    c.insertText('a');
    c.insertPackEmoji(const PackPick(PackKind.emoji, 12, 3, '😀'));
    c.insertText('b');
    await t.pumpWidget(MaterialApp(
      home: Scaffold(body: TextField(controller: c)),
    ));
    await settle(t);
    expect(t.takeException(), isNull);
    expect(find.byType(PackEmojiInline), findsOneWidget);
    expect(c.selection.baseOffset, 3);
    expect(c.encoded, 'a[pe:12:3:😀]b');
  });

  testWidgets('bo\'lmagan stiker o\'rnida belgi turadi', (t) async {
    await t.pumpWidget(const MaterialApp(
      home: Scaffold(body: PackMediaView(file: 'pk_5_9', type: 'sticker')),
    ));
    await settle(t);
    expect(t.takeException(), isNull);
    expect(find.text('Stiker'), findsOneWidget);
  });

  testWidgets('buzuq havola ham xato bermaydi', (t) async {
    await t.pumpWidget(const MaterialApp(
      home: Scaffold(body: PackMediaView(file: 'video.mp4', type: 'gif')),
    ));
    expect(t.takeException(), isNull);
    expect(find.text('GIF'), findsOneWidget);
  });
}
