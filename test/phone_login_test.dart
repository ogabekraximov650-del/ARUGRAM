import 'package:flutter_test/flutter_test.dart';
import 'package:soft/screens/phone_login_screen.dart';

String _type(String old, String now) => PhoneNumberFormatter()
    .formatEditUpdate(
        TextEditingValue(text: old), TextEditingValue(text: now))
    .text;

void main() {
  group('Raqam maydoni', () {
    test('boshida doim + turadi', () {
      expect(_type('+', '+9'), '+9');
      expect(_type('+99', '+998'), '+998');
    });

    test("+ ni o'chirib bo'lmaydi", () {
      expect(_type('+', ''), '+');
      expect(_type('+9', '9'), '+9');
    });

    test('faqat raqam qabul qilinadi', () {
      expect(_type('+998', '+998 (90) 123-45-67'), '+998901234567');
      expect(_type('+', '+abc'), '+');
    });

    test('15 raqamdan oshmaydi', () {
      expect(_type('+', '+12345678901234567'), '+123456789012345');
    });
  });
}
