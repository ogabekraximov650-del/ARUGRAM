// lib/widgets/tg_reply.dart — XABARGA JAVOB (Telegram'dagi "Reply").
//
// Telegram'da xabarni chapga surish yoki menyudan "Javob berish" —
// yozish panelining tepasida javob qatori (`ReplyMessageLine`) chiqadi,
// yuborilgan xabarning tepasida esa asl xabar iqtibosi (chapda chiziq,
// yuboruvchi nomi va qisqa matn) turadi; iqtibos bosilsa asl xabarga
// o'tiladi va u bir zum yoritiladi.
//
// Serverda alohida ustun yo'q (sxema o'zgartirilmaydi) — javob xabar
// matnining boshida `[re:<xabar id>]` bo'lib turadi; ro'yxatdagi oxirgi
// xabar yozuvida worker uni olib tashlaydi.

import 'package:flutter/material.dart';

final _re = RegExp(r'^\[re:([A-Za-z0-9_-]{1,40})\]');

/// `(asl xabar id, qolgan matn)`.
(String?, String) tgSplitReply(String body) {
  final m = _re.firstMatch(body);
  if (m == null) return (null, body);
  return (m.group(1), body.substring(m.end));
}

String tgWithReply(String? id, String text) =>
    id == null || id.isEmpty ? text : '[re:$id]$text';

/// Iqtibos (`ReplyMessageLine`): chapda 3 dp chiziq, nom (14, qalin) va
/// bir qator matn (14).
class TgReplyQuote extends StatelessWidget {
  final String name;
  final String text;
  final Color color;
  final Color background;
  final VoidCallback? onTap;

  const TgReplyQuote({
    super.key,
    required this.name,
    required this.text,
    required this.color,
    required this.background,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: onTap,
      child: Container(
        margin: const EdgeInsets.only(bottom: 4),
        decoration: BoxDecoration(
          color: background,
          borderRadius: BorderRadius.circular(6),
        ),
        child: IntrinsicHeight(
          child: Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Container(
                width: 3,
                decoration: BoxDecoration(
                  color: color,
                  borderRadius: const BorderRadius.horizontal(
                      left: Radius.circular(6)),
                ),
              ),
              Flexible(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(7, 4, 8, 5),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                              color: color,
                              fontSize: 14,
                              fontWeight: FontWeight.w600)),
                      Text(text,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              color: Colors.white, fontSize: 14)),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Yozish panelining tepasidagi javob qatori: ↩ belgi, nom va matn, ✕.
class TgReplyBar extends StatelessWidget {
  final String name;
  final String text;
  final Color color;
  final VoidCallback onClose;

  const TgReplyBar({
    super.key,
    required this.name,
    required this.text,
    required this.color,
    required this.onClose,
  });

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 48,
      child: Row(
        children: [
          const SizedBox(width: 14),
          Icon(Icons.reply_rounded, color: color, size: 24),
          const SizedBox(width: 12),
          Container(width: 2, height: 32, color: color),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                        color: color,
                        fontSize: 14,
                        fontWeight: FontWeight.w600)),
                Text(text,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                        color: Colors.white.withValues(alpha: 0.75),
                        fontSize: 14)),
              ],
            ),
          ),
          IconButton(
            onPressed: onClose,
            icon: Icon(Icons.close_rounded,
                color: Colors.white.withValues(alpha: 0.6)),
          ),
        ],
      ),
    );
  }
}

/// Xabarni chapga surib javob berish (Telegram): pufak barmoq bilan
/// siljiydi, o'ngda ↩ doira paydo bo'ladi; yetarli surilsa (50 dp)
/// tebranadi va qo'yib yuborilganda [onReply] chaqiriladi.
class TgSwipeReply extends StatefulWidget {
  final Widget child;
  final VoidCallback? onReply;

  const TgSwipeReply({super.key, required this.child, this.onReply});

  @override
  State<TgSwipeReply> createState() => _TgSwipeReplyState();
}

class _TgSwipeReplyState extends State<TgSwipeReply>
    with SingleTickerProviderStateMixin {
  static const _trigger = 50.0;
  double _dx = 0;
  bool _armed = false;
  late final AnimationController _back = AnimationController(
      vsync: this, duration: const Duration(milliseconds: 200))
    ..addListener(() => setState(() => _dx = _from * (1 - _back.value)));
  double _from = 0;

  @override
  void dispose() {
    _back.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.onReply == null) return widget.child;
    final p = (-_dx / _trigger).clamp(0.0, 1.0);
    return GestureDetector(
      onHorizontalDragStart: (_) {
        _back.stop();
        _armed = false;
      },
      onHorizontalDragUpdate: (d) {
        setState(() => _dx = (_dx + d.delta.dx).clamp(-90.0, 0.0));
        final armed = -_dx >= _trigger;
        if (armed != _armed) {
          _armed = armed;
          if (armed) Feedback.forLongPress(context);
        }
      },
      onHorizontalDragEnd: (_) {
        if (_armed) widget.onReply!();
        _armed = false;
        _from = _dx;
        _back.forward(from: 0);
      },
      child: Stack(
        clipBehavior: Clip.none,
        alignment: Alignment.centerRight,
        children: [
          Positioned(
            right: 8,
            child: Opacity(
              opacity: p,
              child: Transform.scale(
                scale: 0.5 + 0.5 * p,
                child: Container(
                  width: 32,
                  height: 32,
                  decoration: const BoxDecoration(
                    color: Color(0x66000000),
                    shape: BoxShape.circle,
                  ),
                  child: const Icon(Icons.reply_rounded,
                      size: 20, color: Colors.white),
                ),
              ),
            ),
          ),
          Transform.translate(offset: Offset(_dx, 0), child: widget.child),
        ],
      ),
    );
  }
}
