// lib/services/native_pool.dart — Rust yadrosi uchun DOIMIY ishchi
// isolate'lar.
//
// Har chaqiruvda `Isolate.run` yangi isolate ochib yopardi; bu yerda
// bir necha doimiy ishchi bor. `NativePool.io` — tarmoqqa chiqadigan
// Telegram chaqiruvlari.
//
// Telegram stiker/emoji/GIF chizish (rlottie/libvpx) hovuzlari olib
// tashlandi (ilova o'z tizimini yasaydi).

import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

import 'device_perf.dart';

DynamicLibrary _lib() => Platform.isAndroid
    ? DynamicLibrary.open('librust_core.so')
    : DynamicLibrary.process();

class NativePool {
  final int size;
  final String name;
  NativePool._(this.size, this.name);

  // XOTIRA: har isolate o'z uyumi bilan bir necha MB egallaydi —
  // kuchsiz (2 GB) telefonda ishchilar kamroq.
  static final io = NativePool._(DevicePerf.low ? 2 : 3, 'aru-io');

  final List<_Worker> _workers = [];
  int _next = 0;

  _Worker _pick() {
    while (_workers.length < size) {
      _workers.add(_Worker('$name-${_workers.length}'));
    }
    // Eng BO'SH ishchi (navbat bo'yicha emas): band ishchiga qo'yilgan
    // tezkor so'rov uning sekin ishi tugashini kutib qolardi.
    var best = _workers[_next++ % size];
    for (final w in _workers) {
      if (w.busy < best.busy) best = w;
    }
    return best;
  }

  /// JSON qaytaradigan Rust funksiyasi: argumentsiz, satr ([arg]) yoki
  /// son ([intArg]) bilan.
  Future<Map<String, dynamic>> call(String fn,
      {String? arg, int? intArg}) async {
    final r = await _pick()
        .request({'op': 'call', 'fn': fn, 'arg': arg, 'int': intArg});
    if (r is String) {
      try {
        final v = jsonDecode(r);
        if (v is Map<String, dynamic>) return v;
      } catch (_) {}
      return {'error': r.isEmpty ? 'Javob yo\'q' : r};
    }
    return {'error': '$r'};
  }
}

class _Worker {
  final String name;
  SendPort? _port;
  final Completer<SendPort> _ready = Completer();
  final ReceivePort _rx = ReceivePort();
  final Map<int, Completer<Object?>> _pending = {};
  int _seq = 0;

  /// Javobi kutilayotgan so'rovlar soni.
  int get busy => _pending.length;

  _Worker(this.name) {
    _rx.listen((m) {
      if (m is SendPort) {
        _ready.complete(m);
        return;
      }
      if (m is List && m.length == 2) {
        _pending.remove(m[0] as int)?.complete(m[1]);
      }
    });
    Isolate.spawn(_main, _rx.sendPort, debugName: name).catchError((e) {
      if (!_ready.isCompleted) _ready.completeError(e);
      return Isolate.current;
    });
  }

  Future<Object?> request(Map<String, Object?> msg) async {
    final port = _port ??= await _ready.future;
    final id = ++_seq;
    final c = Completer<Object?>();
    _pending[id] = c;
    port.send([id, msg]);
    return c.future;
  }
}

typedef _NoArgC = Pointer<Utf8> Function();
typedef _StrC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _IntC = Pointer<Utf8> Function(Int32);
typedef _IntD = Pointer<Utf8> Function(int);
typedef _FreeC = Void Function(Pointer<Utf8>);
typedef _FreeD = void Function(Pointer<Utf8>);

void _main(SendPort out) {
  final rx = ReceivePort();
  out.send(rx.sendPort);
  final lib = _lib();
  final free = lib.lookupFunction<_FreeC, _FreeD>('rust_free_string');
  String take(Pointer<Utf8> p) {
    if (p == nullptr) return '';
    final s = p.toDartString();
    free(p);
    return s;
  }

  rx.listen((raw) {
    final m = raw as List;
    final id = m[0] as int;
    final msg = (m[1] as Map).cast<String, Object?>();
    Object? result;
    try {
      switch (msg['op']) {
        case 'call':
          final fn = msg['fn'] as String;
          final arg = msg['arg'] as String?;
          final i = msg['int'] as int?;
          if (arg != null) {
            final a = arg.toNativeUtf8();
            try {
              result = take(lib.lookupFunction<_StrC, _StrC>(fn)(a));
            } finally {
              malloc.free(a);
            }
          } else if (i != null) {
            result = take(lib.lookupFunction<_IntC, _IntD>(fn)(i));
          } else {
            result = take(lib.lookupFunction<_NoArgC, _NoArgC>(fn)());
          }
      }
    } catch (e) {
      result = msg['op'] == 'call' ? '{"error":"$e"}' : null;
    }
    out.send([id, result]);
  });
}
