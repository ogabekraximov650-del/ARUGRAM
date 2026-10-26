---
name: Duplicate post fix
description: Kanalga post ikki marta yuborilish muammosi va yechimi
---

## Muammo
24 ta qismli animeda 9-chi va 17-chi postlar qayta joylandi.
Sabab: tgSend muvaffaqiyatli, lekin markQueueItemDone() DB yozish xato bersa — item 3 daqiqadan keyin reset qilinib qayta yuborilardi.

## Yechim (post_queue.posted_at)
1. markQueueItemSending(id) — tgSend DAN OLDIN posted_at yozadi (DB xatosi ignore qilinadi)
2. markQueueItemDone(id) — posted_at = COALESCE(posted_at, now), status='done'
3. resetAllProcessingItems() — faqat posted_at IS NULL bo'lgan itemlarni reset qiladi
4. getPendingQueueItems() da posted_at SELECT qilinadi — processing da ham posted_at bor bo'lsa skip

## Schema
post_queue jadvalida: processing_started_at TEXT, posted_at TEXT
ALTER TABLE orqali mavjud bazaga ham qo'shiladi (xato bo'lsa ignore).

**Why:** posted_at = "yuborilganlik belgisi". Bu null bo'lmasa, item hech qachon qayta yuborilmaydi.
