// lib/services/native_pool.dart — Rust yadrosi uchun DOIMIY ishchi
// isolate'lar.
//
// TOPILGAN SABAB (foydalanuvchi: "to'plamlarni ochishim bilan ilova
// qotib qolyapti"): panel har bir stiker fayli va har bir to'plam
// uchun `Isolate.run` chaqirardi — ya'ni HAR SAFAR yangi isolate
// ochilib yopilardi. Panel ochilganda bu yuzlab isolate degani.
// Telegram/Cherrygram esa bir necha doimiy fon oqimida ishlaydi
// (`RLottieDrawable`: 4 ta oqim, `DispatchQueue`).
//
// Endi ikki hovuz bor:
//   * `NativePool.io`     — tarmoqqa chiqadigan chaqiruvlar (stiker
//                           to'plami, fayl yuklash) — 3 ta ishchi;
//   * `NativePool.render` — stiker kadrlari (rlottie / libvpx) — 2 ta
//                           ishchi; tarmoq kutayotgan chaqiruv
//                           animatsiyani to'xtatib qo'ymasin.

import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

DynamicLibrary _lib() => Platform.isAndroid
    ? DynamicLibrary.open('librust_core.so')
    : DynamicLibrary.process();

class NativePool {
  final int size;
  final String name;
  NativePool._(this.size, this.name);

  static final io = NativePool._(3, 'aru-io');
  static final render = NativePool._(2, 'aru-render');

  final List<_Worker> _workers = [];
  int _next = 0;

  _Worker _pick([int? pin]) {
    while (_workers.length < size) {
      _workers.add(_Worker('$name-${_workers.length}'));
    }
    if (pin != null) return _workers[pin % size];
    return _workers[_next++ % size];
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

  /// Animatsiyani ochadi: `(tutqich, kadrlar, fps)` yoki `null`.
  Future<(int, int, double)?> animOpen(String path, int w, int h) async {
    final r =
        await _pick().request({'op': 'open', 'path': path, 'w': w, 'h': h});
    if (r is List && r.length == 3 && (r[0] as int) > 0) {
      return (r[0] as int, r[1] as int, (r[2] as num).toDouble());
    }
    return null;
  }

  /// Kadr (premultiplied RGBA, w*h*4) yoki `null`. Bir animatsiyaning
  /// hamma kadri BITTA ishchida chiziladi (VP9 ketma-ket ochiladi).
  Future<Uint8List?> animFrame(int handle, int frame, int w, int h) async {
    final r = await _pick(handle).request(
        {'op': 'frame', 'h': handle, 'f': frame, 'w': w, 'hh': h});
    if (r is TransferableTypedData) return r.materialize().asUint8List();
    return null;
  }

  void animClose(int handle) {
    _pick(handle).request({'op': 'close', 'h': handle});
  }
}

class _Worker {
  final String name;
  SendPort? _port;
  final Completer<SendPort> _ready = Completer();
  final ReceivePort _rx = ReceivePort();
  final Map<int, Completer<Object?>> _pending = {};
  int _seq = 0;

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
typedef _OpenC = Int64 Function(Pointer<Utf8>, Int32, Int32);
typedef _OpenD = int Function(Pointer<Utf8>, int, int);
typedef _I64IntC = Int32 Function(Int64);
typedef _I64IntD = int Function(int);
typedef _FpsC = Double Function(Int64);
typedef _FpsD = double Function(int);
typedef _RenderC = Int32 Function(Int64, Int32, Pointer<Uint8>);
typedef _RenderD = int Function(int, int, Pointer<Uint8>);
typedef _CloseC = Void Function(Int64);
typedef _CloseD = void Function(int);

void _main(SendPort out) {
  final rx = ReceivePort();
  out.send(rx.sendPort);
  final lib = _lib();
  final free = lib.lookupFunction<_FreeC, _FreeD>('rust_free_string');
  final open = lib.lookupFunction<_OpenC, _OpenD>('rust_anim_open');
  final frames = lib.lookupFunction<_I64IntC, _I64IntD>('rust_anim_frames');
  final fps = lib.lookupFunction<_FpsC, _FpsD>('rust_anim_fps');
  final render = lib.lookupFunction<_RenderC, _RenderD>('rust_anim_render');
  final close = lib.lookupFunction<_CloseC, _CloseD>('rust_anim_close');
  // Har bir animatsiya uchun bitta bufer (qayta ishlatiladi).
  final bufs = <int, (Pointer<Uint8>, int)>{};

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
        case 'open':
          final p = (msg['path'] as String).toNativeUtf8();
          try {
            final h = open(p, msg['w'] as int, msg['h'] as int);
            result = h > 0 ? [h, frames(h), fps(h)] : [0, 0, 0.0];
          } finally {
            malloc.free(p);
          }
        case 'frame':
          final h = msg['h'] as int;
          final len = (msg['w'] as int) * (msg['hh'] as int) * 4;
          var b = bufs[h];
          if (b == null || b.$2 != len) {
            if (b != null) malloc.free(b.$1);
            b = bufs[h] = (malloc<Uint8>(len), len);
          }
          final r = render(h, msg['f'] as int, b.$1);
          result = r == 1
              ? TransferableTypedData.fromList([b.$1.asTypedList(len)])
              : null;
        case 'close':
          final h = msg['h'] as int;
          close(h);
          final b = bufs.remove(h);
          if (b != null) malloc.free(b.$1);
      }
    } catch (e) {
      result = msg['op'] == 'call' ? '{"error":"$e"}' : null;
    }
    out.send([id, result]);
  });
}
