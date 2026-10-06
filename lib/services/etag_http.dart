// lib/services/etag_http.dart — "O'ZGARISH BORMI?" BILAN GET.
//
// TALAB (foydalanuvchi): "kesh faqat o'zgarish bor-yo'qligini aytsin;
// bor bo'lsagina worker Turso'ga so'rov yuborsin".
//
// Worker ba'zi ro'yxatlarga versiya (`ETag`) qo'yadi — u o'qiladigan
// jadvallarning o'zgarish belgisidan (`MarkHub`) olinadi. Ilova javobni
// versiyasi bilan diskda saqlaydi va keyingi safar versiyani
// `If-None-Match` da yuboradi. O'zgarish bo'lmasa worker 304 (bo'sh)
// qaytaradi va Turso'ga BORMAYDI — bu yerda diskdagi javob xuddi
// serverdan kelgandek (200) qaytariladi, ya'ni chaqiruvchi kod
// o'zgarmaydi.
//
// Saqlash shifrlangan (`saveListCache`) va hisob papkasida; kalitda
// hisob raqami ham bor — boshqa hisobning javobi berilmaydi.

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'auth_service.dart';
import 'rust_bridge.dart';

class EtagHttp {
  const EtagHttp._();

  static Future<http.Response> get(Uri uri,
      {Map<String, String>? headers}) async {
    final key = _key(uri);
    Map<String, dynamic>? saved;
    try {
      final l = RustCore.instance.getCachedList(key);
      if (l != null && l.isNotEmpty) saved = l.first;
    } catch (_) {}
    final tag = saved?['e'];
    final body = saved?['b'];
    final h = <String, String>{...?headers};
    if (tag is String && tag.isNotEmpty && body is String) {
      h['If-None-Match'] = tag;
    }
    final r = await http.get(uri, headers: h);
    if (r.statusCode == 304 && body is String) {
      return http.Response.bytes(utf8.encode(body), 200,
          headers: {'content-type': 'application/json; charset=utf-8'});
    }
    if (r.statusCode == 200) {
      final e = r.headers['etag'] ?? '';
      try {
        if (e.isNotEmpty) {
          RustCore.instance.saveListCache(key, [
            {'e': e, 'b': utf8.decode(r.bodyBytes, allowMalformed: true)}
          ]);
        }
      } catch (_) {}
    }
    return r;
  }

  static String _key(Uri u) {
    final s = '${AuthService.instance.user?.id ?? 0}|$u';
    var h = 0xcbf29ce484222325;
    for (final c in utf8.encode(s)) {
      h ^= c;
      h = (h * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF;
    }
    return 'etag_${h.toUnsigned(64).toRadixString(16)}';
  }
}
