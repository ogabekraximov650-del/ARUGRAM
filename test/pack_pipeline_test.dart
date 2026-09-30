// test/pack_pipeline_test.dart — TO'PLAM FAYLIDAN RASM OLISH (Telegram'siz).
//
// `test/fixtures/sample_pack.arp` — Python (`tool/packs/arupack.py`) yasagan
// HAQIQIY to'plam fayli (shifrlanmagan). `PackService` uni xuddi Telegram'dan
// oraliq (Range) bilan o'qigandek o'qiydi: sarlavha, kichik rasmlar,
// elementlar. Format Python va Dart o'rtasida to'g'ri kelishini tekshiradi.

import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:soft/services/pack_service.dart';

void main() {
  final file = File('test/fixtures/sample_pack.arp').readAsBytesSync();
  final svc = PackService.instance;
  var reads = 0;

  setUp(() {
    reads = 0;
    svc.rangeOverride = (name, off, len) async {
      reads++;
      if (off >= file.length) return null;
      final end = (off + len) > file.length ? file.length : off + len;
      return Uint8List.sublistView(file, off, end);
    };
  });

  tearDown(() => svc.rangeOverride = null);

  const info = PackInfo(
      id: 77, kind: 'sticker', title: 'Sinov', file: 'pk_77_2.arp', version: 2, items: 3);

  test('sarlavha o\'qiladi', () async {
    final h = await svc.header(info);
    expect(h, isNotNull);
    expect(h!.items.length, 3);
    expect(h.ver, 2);
    expect(h.find(1)!.emoji, '😀');
  });

  test('kichik rasm va element haqiqiy WebP', () async {
    final h = (await svc.header(info))!;
    final ref = PackRef(info, h, h.find(2)!);
    final t = await svc.thumb(ref);
    final d = await svc.data(ref);
    expect(t, isNotNull);
    expect(d, isNotNull);
    expect(sniffImage(t!), 'webp');
    expect(sniffImage(d!), 'webp');
    expect(t.length, ref.item.thumbLen);
    expect(d.length, ref.item.len);
  });

  test('kichik to\'plam bitta o\'qishda hammasi keshga tushadi', () async {
    final i2 = PackInfo(
        id: 78, kind: 'sticker', title: 'Sinov 2', file: 'pk_78_2.arp', version: 2, items: 3);
    // Sarlavhadagi id 77 — boshqa to'plam nomi bilan ochilmaydi.
    expect(await svc.header(i2), isNull);
    final h = (await svc.header(info))!;
    reads = 0;
    for (final it in h.items) {
      final r = PackRef(info, h, it);
      expect(await svc.thumb(r), isNotNull);
      expect(await svc.data(r), isNotNull);
    }
    expect(reads, 0, reason: 'birinchi o\'qishdayoq keshga tushgan bo\'lishi kerak');
  });
}
