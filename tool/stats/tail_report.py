#!/usr/bin/env python3
"""`wrangler tail --format json` chiqishidan hisobot.

Worker har Turso buyrug'idan keyin `aru_tq r=<o'qilgan> w=<yozilgan> <SQL>`
qatorini yozadi (`worker/src/lib.rs` -> `turso_after`). Bu skript tail
hodisalarini o'qib, quyidagilarni Markdown jadval qilib chiqaradi:

  * yo'llar (`GET /api/...`): so'rovlar soni, Turso buyruqlari,
    o'qilgan va yozilgan qatorlar;
  * SQL buyruqlari: shu ko'rsatkichlar bo'yicha eng qimmatlari;
  * cron (`scheduled`) hodisalari alohida.

Ishlatish: python3 tail_report.py tail.jsonl <daqiqa> > report.md
"""
import json
import re
import sys
from collections import defaultdict

NUM = re.compile(r"/\d+(?=/|$)")
TQ = re.compile(r"^aru_tq r=(-?\d+) w=(-?\d+) (.*)$")
SV = re.compile(r"^aru_sv (\S+) (\d+)$")

SV_NAMES = {
    "chat": "Support chat kutishi (kesh belgisi)",
    "nuqta": "O'qilmaganlar nuqtasi (kesh belgisi)",
    "izohlar": "Izohlar ro'yxati o'zgarmagan (kesh belgisi)",
    "sxema": "Jadval tekshiruvi o'tkazildi (kesh belgisi)",
    "sozlama": "Sozlama xotiradan olindi",
    "kanal": "Yopiq kanal so'rovi (bitta buyruq)",
}


def events(text):
    dec = json.JSONDecoder()
    i, n = 0, len(text)
    while i < n:
        while i < n and text[i] not in "{":
            i += 1
        if i >= n:
            break
        try:
            obj, j = dec.raw_decode(text, i)
        except json.JSONDecodeError:
            i += 1
            continue
        i = j
        if isinstance(obj, dict):
            yield obj


def path_of(ev):
    e = ev.get("event") or {}
    req = e.get("request")
    if isinstance(req, dict):
        url = req.get("url", "")
        path = re.sub(r"^https?://[^/]+", "", url).split("?")[0] or "/"
        path = NUM.sub("/:n", path)
        return f"{req.get('method', '?')} {path}"
    if "cron" in e or "scheduledTime" in e:
        return f"CRON {e.get('cron', '')}".strip()
    return "boshqa"


def main():
    raw = open(sys.argv[1], encoding="utf-8", errors="replace").read()
    minutes = float(sys.argv[2]) if len(sys.argv) > 2 else 0
    by_path = defaultdict(lambda: [0, 0, 0, 0])  # so'rov, buyruq, o'qish, yozish
    by_sql = defaultdict(lambda: [0, 0, 0])  # buyruq, o'qish, yozish
    total = [0, 0, 0, 0]
    by_sv = defaultdict(lambda: [0, 0])  # holat, tejalgan buyruq
    for ev in events(raw):
        p = path_of(ev)
        row = by_path[p]
        row[0] += 1
        total[0] += 1
        for log in ev.get("logs") or []:
            msg = log.get("message")
            if isinstance(msg, list):
                msg = " ".join(str(m) for m in msg)
            sv = SV.match(str(msg or ""))
            if sv:
                by_sv[sv.group(1)][0] += 1
                by_sv[sv.group(1)][1] += int(sv.group(2))
                continue
            m = TQ.match(str(msg or ""))
            if not m:
                continue
            r, w, sql = max(int(m.group(1)), 0), max(int(m.group(2)), 0), m.group(3)
            row[1] += 1
            row[2] += r
            row[3] += w
            s = by_sql[sql]
            s[0] += 1
            s[1] += r
            s[2] += w
            total[1] += 1
            total[2] += r
            total[3] += w

    out = []
    out.append(f"# Worker va Turso hisoboti ({minutes:g} daqiqa)\n")
    out.append(
        f"Jami: **{total[0]}** so'rov, **{total[1]}** Turso buyrug'i, "
        f"**{total[2]}** o'qilgan qator, **{total[3]}** yozilgan qator.\n"
    )
    if minutes > 0:
        k = 60 * 24 / minutes
        out.append(
            f"Kunlik taxmin (shu sur'atda): ~{int(total[0] * k)} so'rov, "
            f"~{int(total[2] * k)} o'qish, ~{int(total[3] * k)} yozish.\n"
        )
    saved_total = sum(v[1] for v in by_sv.values())
    out.append("\n## Tejamkor tizim: Turso'ga borilmagan holatlar\n")
    if by_sv:
        out.append("| Tejash | Necha marta | Tejalgan Turso buyrug'i |")
        out.append("|---|---:|---:|")
        for k, v in sorted(by_sv.items(), key=lambda kv: -kv[1][1]):
            out.append(f"| {SV_NAMES.get(k, k)} | {v[0]} | {v[1]} |")
        done = total[1]
        if done + saved_total > 0:
            pct = 100 * saved_total / (done + saved_total)
            out.append(
                f"\nJami tejaldi: **{saved_total}** buyruq — tejamsiz usulda "
                f"{done + saved_total} bo'lardi, haqiqatda {done} ketdi "
                f"(**{pct:.1f}%** kam).\n"
            )
        if minutes > 0:
            out.append(f"Kunlik taxmin: ~{int(saved_total * 60 * 24 / minutes)} buyruq tejaladi.\n")
    else:
        out.append("Bu oraliqda tejash holati qayd etilmadi (foydalanuvchilar faol emas edi).\n")

    base = None
    try:
        import os
        bp = os.path.join(os.path.dirname(os.path.abspath(__file__)), "baseline.json")
        base = json.load(open(bp, encoding="utf-8"))
    except Exception:
        base = None
    if base and minutes > 0 and base.get("minutes"):
        bm = float(base["minutes"])
        out.append("\n## Tejamkor tizimdan OLDINGI o'lchov bilan solishtirish (daqiqasiga)\n")
        out.append(f"_{base.get('izoh', '')}_\n")
        out.append("| Ko'rsatkich | Oldin | Hozir | Farq |")
        out.append("|---|---:|---:|---:|")
        for name, key, now_v in [
            ("So'rov", "requests", total[0]),
            ("Turso buyrug'i", "turso", total[1]),
            ("O'qilgan qator", "read", total[2]),
            ("Yozilgan qator", "written", total[3]),
        ]:
            b = base[key] / bm
            n = now_v / minutes
            diff = f"{100 * (n - b) / b:+.0f}%" if b > 0 else "—"
            out.append(f"| {name} | {b:.1f} | {n:.1f} | {diff} |")
        out.append(
            "\nEslatma: yuklama vaqtga qarab o'zgaradi (foydalanuvchilar soni), "
            "shu sabab bu solishtirish taxminiy; aniq tejashni yuqoridagi jadval ko'rsatadi.\n"
        )

    out.append("\n## Yo'llar (o'qilgan qator bo'yicha)\n")
    out.append("| Yo'l | So'rov | Turso buyruq | O'qilgan | Yozilgan |")
    out.append("|---|---:|---:|---:|---:|")
    for p, v in sorted(by_path.items(), key=lambda kv: (-kv[1][2], -kv[1][0]))[:40]:
        out.append(f"| `{p}` | {v[0]} | {v[1]} | {v[2]} | {v[3]} |")
    out.append("\n## Yo'llar (so'rovlar soni bo'yicha)\n")
    out.append("| Yo'l | So'rov | Turso buyruq |")
    out.append("|---|---:|---:|")
    for p, v in sorted(by_path.items(), key=lambda kv: -kv[1][0])[:25]:
        out.append(f"| `{p}` | {v[0]} | {v[1]} |")
    out.append("\n## SQL (o'qilgan qator bo'yicha)\n")
    out.append("| SQL boshi | Marta | O'qilgan | Yozilgan |")
    out.append("|---|---:|---:|---:|")
    for q, v in sorted(by_sql.items(), key=lambda kv: (-kv[1][1], -kv[1][0]))[:30]:
        safe = q.replace("|", "\\|")
        out.append(f"| `{safe}` | {v[0]} | {v[1]} | {v[2]} |")
    out.append("\n## SQL (yozilgan qator bo'yicha)\n")
    out.append("| SQL boshi | Marta | Yozilgan |")
    out.append("|---|---:|---:|")
    for q, v in sorted(by_sql.items(), key=lambda kv: -kv[1][2])[:15]:
        if v[2] <= 0:
            break
        safe = q.replace("|", "\\|")
        out.append(f"| `{safe}` | {v[0]} | {v[2]} |")
    print("\n".join(out))


if __name__ == "__main__":
    main()
