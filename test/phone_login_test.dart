import 'package:flutter_test/flutter_test.dart';
import 'package:soft/widgets/tg_countries.dart';

void main() {
  group('Davlatlar', () {
    test("O'zbekiston raqami bo'laklanadi", () {
      final uz = countryByIso('UZ')!;
      expect(uz.code, '998');
      expect(uz.format('901234567'), '90 123 45 67');
      expect(uz.format('9012'), '90 12');
      expect(uz.length, 9);
    });

    test("to'liq raqam davlat va raqamga ajraladi", () {
      final (c, rest) = splitPhone('998901234567');
      expect(c?.iso, 'UZ');
      expect(rest, '901234567');
    });

    test('umumiy kodda asosiy davlat tanlanadi', () {
      expect(countryByCode('7')?.iso, 'RU');
      expect(countryByCode('1')?.iso, 'US');
      final kz = countryByIso('KZ');
      expect(countryByCode('7', prefer: kz)?.iso, 'KZ');
    });

    test('bayroq emojisi', () {
      expect(countryByIso('UZ')!.flag, '\u{1F1FA}\u{1F1FF}');
    });

    test("ro'yxat to'liq", () {
      expect(tgCountries.length, greaterThan(200));
    });
  });
}
