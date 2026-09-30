// lib/widgets/pack_views.dart — TO'PLAM ELEMENTLARINI KO'RSATISH.
//
// Telefonga bosim tushmasligi uchun (`pack_service.dart` boshidagi izoh):
//
//   * ro'yxat va to'plam oynasida faqat kichik STATIK rasm (`PackImage`,
//     `animate: false`);
//   * xabarda va matn ichida animatsiya faqat bir vaqtda cheklangan sondagi
//     joyda (`AnimSlots`) — qolgani statik turadi. Telefon kuchsizroq
//     bo'lsa chegara kichikroq (`DevicePerf`);
//   * ekrandan chiqqan vidjet o'z o'rnini qaytaradi;
//   * rasm o'z o'lchamida dekodlanadi (`cacheWidth`) — 512 px lik rasm
//     24 dp lik joyda katta xotira olmaydi.

import 'dart:async';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../services/device_perf.dart';
import '../services/pack_service.dart';
import 'media_placeholder.dart';

/// Bir vaqtda nechta animatsiya ishlashi mumkin.
class AnimSlots {
  AnimSlots._();
  static final AnimSlots instance = AnimSlots._();

  final Set<Object> _used = {};

  int get max => switch (DevicePerf.cls) {
        PerfClass.low => 2,
        PerfClass.average => 5,
        PerfClass.high => 9,
      };

  bool tryAcquire(Object owner) {
    if (_used.contains(owner)) return true;
    if (_used.length >= max) return false;
    _used.add(owner);
    return true;
  }

  void release(Object owner) => _used.remove(owner);
}

/// Bitta element rasmi.
///
/// [animate] `false` — faqat kichik statik rasm. `true` — elementning o'zi;
/// animatsiyali bo'lsa va bo'sh joy ([AnimSlots]) bo'lsa animatsiya
/// ishlaydi, aks holda statik rasm turadi.
class PackImage extends StatefulWidget {
  final int pack;
  final int item;
  final double size;
  final bool animate;
  final BoxFit fit;

  /// Yuklanmasa (yoki to'plam topilmasa) ko'rsatiladigan narsa.
  final Widget? fallback;

  const PackImage({
    super.key,
    required this.pack,
    required this.item,
    required this.size,
    this.animate = false,
    this.fit = BoxFit.contain,
    this.fallback,
  });

  @override
  State<PackImage> createState() => _PackImageState();
}

class _PackImageState extends State<PackImage> {
  Uint8List? _bytes;
  bool _failed = false;
  bool _slot = false;
  int _gen = 0;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  @override
  void didUpdateWidget(PackImage old) {
    super.didUpdateWidget(old);
    if (old.pack != widget.pack ||
        old.item != widget.item ||
        old.animate != widget.animate) {
      _release();
      _bytes = null;
      _failed = false;
      unawaited(_load());
    }
  }

  @override
  void dispose() {
    _gen++;
    _release();
    super.dispose();
  }

  void _release() {
    if (_slot) {
      AnimSlots.instance.release(this);
      _slot = false;
    }
  }

  Future<void> _load() async {
    final gen = ++_gen;
    final svc = PackService.instance;
    final r = await svc.resolve(widget.pack, widget.item);
    if (!mounted || gen != _gen) return;
    if (r == null) {
      setState(() => _failed = true);
      return;
    }
    // 1) tez: kichik statik rasm.
    final t = await svc.thumb(r);
    if (!mounted || gen != _gen) return;
    if (t != null) setState(() => _bytes = t);
    if (!widget.animate) {
      if (t == null) setState(() => _failed = true);
      return;
    }
    // 2) elementning o'zi: animatsiya bo'lsa faqat bo'sh joy bo'lganda,
    // statik bo'lsa doim (sifatliroq).
    if (r.item.animated) {
      if (!AnimSlots.instance.tryAcquire(this)) return;
      _slot = true;
    }
    final d = await svc.data(r);
    if (!mounted || gen != _gen) return;
    if (d != null) {
      setState(() => _bytes = d);
    } else {
      _release();
      if (_bytes == null) setState(() => _failed = true);
    }
  }

  @override
  Widget build(BuildContext context) {
    final b = _bytes;
    if (b == null) {
      if (_failed) {
        return widget.fallback ?? SizedBox(width: widget.size, height: widget.size);
      }
      return SizedBox(width: widget.size, height: widget.size);
    }
    final dpr = MediaQuery.devicePixelRatioOf(context);
    return Image.memory(
      b,
      width: widget.size,
      height: widget.size,
      fit: widget.fit,
      gaplessPlayback: true,
      filterQuality: FilterQuality.low,
      // O'z o'lchamida dekodlanadi — xotira tejaladi.
      cacheWidth: math.max(16, (widget.size * dpr).round()),
      errorBuilder: (_, __, ___) =>
          widget.fallback ?? SizedBox(width: widget.size, height: widget.size),
    );
  }
}

/// Xabar yoki izohdagi stiker / GIF (`pk_<to'plam>_<element>`).
class PackMediaView extends StatelessWidget {
  final String file;
  final String type;
  const PackMediaView({super.key, required this.file, required this.type});

  @override
  Widget build(BuildContext context) {
    final ref = parsePackRef(file);
    if (ref == null) return MediaPlaceholder(type: type);
    final size = type == 'gif' ? 200.0 : 150.0;
    return PackImage(
      pack: ref.$1,
      item: ref.$2,
      size: size,
      animate: true,
      fallback: MediaPlaceholder(type: type),
    );
  }
}

/// Matn ichidagi maxsus emoji (`[pe:...]`): [size] — matn balandligi.
class PackEmojiInline extends StatelessWidget {
  final int pack;
  final int item;
  final String emoji;
  final double size;
  const PackEmojiInline({
    super.key,
    required this.pack,
    required this.item,
    required this.emoji,
    required this.size,
  });

  @override
  Widget build(BuildContext context) {
    return PackImage(
      pack: pack,
      item: item,
      size: size,
      animate: true,
      // To'plam yuklanmasa oddiy emoji ko'rinadi.
      fallback: SizedBox(
        width: size,
        height: size,
        child: Center(
          child: Text(emoji, style: TextStyle(fontSize: size * 0.8, height: 1)),
        ),
      ),
    );
  }
}

/// `[pe:...]` belgilari bor matnni bo'laklarga ajratadi: oddiy matn
/// [textSpan] orqali, maxsus emoji rasm bo'lib. Belgi bo'lmasa `null`.
List<InlineSpan>? packEmojiSpans(
  String text,
  double fontSize,
  InlineSpan Function(String plain) textSpan,
) {
  if (!text.contains('[pe:')) return null;
  final out = <InlineSpan>[];
  var last = 0;
  for (final m in kPackEmojiToken.allMatches(text)) {
    if (m.start > last) out.add(textSpan(text.substring(last, m.start)));
    out.add(WidgetSpan(
      alignment: PlaceholderAlignment.middle,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 1),
        child: PackEmojiInline(
          pack: int.parse(m.group(1)!),
          item: int.parse(m.group(2)!),
          emoji: m.group(3) ?? '',
          size: fontSize * 1.35,
        ),
      ),
    ));
    last = m.end;
  }
  if (out.isEmpty) return null;
  if (last < text.length) out.add(textSpan(text.substring(last)));
  return out;
}
