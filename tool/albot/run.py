#!/usr/bin/env python3
"""albot: Actions'da mahalliy AI modeli (llama.cpp + gpt-oss-20b) — botga javob beradi.

Run'ni FAQAT worker ishga tushiradi (polling/kutish yo'q): llama-server yuklanadi, worker'dan (`/ai/poll`) javobsiz xabarlar
birin-ketin olinadi, model javob yozadi, javob `/ai/reply` orqali botga yuboriladi. Navbat bo'shagach `/ai/done` deyiladi va run
tugaydi; navbatda xabar qolgan bo'lsa (MAX_MIN tugadi yoki yangi xabar kelgan) worker yangi run ochadi. Chat tarixi Turso'da
(worker orqali); uzun bo'lsa eski qism xulosaga siqiladi.

UZ: Model konteksti 16 384 token — 800 000 token sig'maydi, shuning uchun tarix ~24 000 belgidan oshsa siqiladi.
EN: Context is 16,384 tokens; history over ~24,000 chars is summarised into a short note.
"""
import json
import os
import re
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

API = os.environ["API"].rstrip("/")
RUN_KEY = "".join(os.environ["RUN_KEY"].split())
MODEL = os.environ["MODEL_PATH"]
LLAMA = os.environ["LLAMA_SERVER"]
PORT = 8080
MAX_MIN = float(os.environ.get("MAX_MIN", "45"))
CTX = 24576
HIST_CHARS = 16000   # shundan oshsa tarix siqiladi (qolgan joy vosita natijalariga)
TAIL_CHARS = 5000    # siqishdan keyin shuncha yangi xabar qoladi
MAX_TOKENS = 2500
WORKDIR = os.environ.get("AGENT_WORKDIR", "/mnt/albot/work")   # model buyruqlari shu papkada, alohida (maxfiy kalitlarsiz) foydalanuvchi sifatida ishlaydi
AGENT_USER = os.environ.get("AGENT_USER", "albot")
MAX_STEPS = int(os.environ.get("AGENT_MAX_STEPS", "12"))
AGENT_MAX_MIN = float(os.environ.get("AGENT_MAX_MIN", "25"))
TOOL_OUT = 2500      # vosita natijasidan kontekstga ko'pi bilan shuncha belgi

SYSTEM = (
    "Sen foydalanuvchining shaxsiy yordamchisisan. O'zbek tilida (agar foydalanuvchi boshqa tilda yozmasa) aniq, qisqa va "
    "to'g'ri javob ber. Bilmasang, bilmasligingni ayt.\n\n"
    "MUHIT: sen GitHub Actions'dagi vaqtinchalik Ubuntu mashinasida ishlayapsan (4 CPU, ~4 GB bo'sh RAM, GPU yo'q, internet bor, "
    "python3, pip, ffmpeg, curl, git bor). Mashina har safar yangi: faqat ish papkasidagi fayllar shu suhbat davomida saqlanadi.\n"
    "VOSITALAR (kerak bo'lsa ishlat, oddiy savolga ishlatma):\n"
    "- shell(cmd, timeout): bash buyrug'i, ish papkasida, oddiy foydalanuvchi sifatida (sudo yo'q; pip uchun `pip install --user`).\n"
    "- read_file(path) / write_file(path, content): ish papkasidagi fayllar.\n"
    "- get_attachment(id): foydalanuvchi yuborgan faylni ish papkasiga yuklaydi (xabarda `[Biriktirilgan fayl #N ...]` ko'rinadi, 20 MB gacha).\n"
    "- send_file(path, caption): ish papkasidagi faylni foydalanuvchiga yuboradi (49 MB gacha).\n"
    "QOIDALAR: natijani ko'rmasdan 'tayyor' dema — buyruq natijasini tekshir; xato bo'lsa sababini o'qib tuzat; bir xil xatoni 2 martadan ko'p "
    "takrorlama; uzun ishlarni kichik qadamlarga bo'l; tayyor fayllarni send_file bilan yubor; oxirida nima qilganingni 1–3 jumlada ayt. "
    "Maxfiy ma'lumot (token, parol) so'ralsa yoki mashinadan tashqariga zarar yetkazadigan ish so'ralsa, rad et."
)

T0 = time.time()


def log(*a):
    print(f"[{int(time.time() - T0):>5}s]", *a, flush=True)


def call(path, body=None, timeout=60):
    data = json.dumps(body or {}).encode()
    req = urllib.request.Request(
        API + path, data=data, method="POST",
        headers={"X-Run-Key": RUN_KEY, "Content-Type": "application/json", "User-Agent": "albot-run"},
    )
    last = None
    for i in range(4):
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return json.loads(r.read().decode() or "{}")
        except urllib.error.HTTPError as e:
            if e.code < 500:
                raise
            last = e
        except Exception as e:  # tarmoq
            last = e
        time.sleep(2 * (i + 1))
    raise RuntimeError(f"{path}: {last}")


def llm_raw(messages, max_tokens=MAX_TOKENS, effort="low", tools=None):
    body = {
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": 0.7,
        "chat_template_kwargs": {"reasoning_effort": effort},
    }
    if tools:
        body["tools"] = tools
    req = urllib.request.Request(
        f"http://127.0.0.1:{PORT}/v1/chat/completions",
        data=json.dumps(body).encode(), method="POST", headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=1500) as r:
        return json.loads(r.read().decode())


def llm(messages, max_tokens=MAX_TOKENS, effort="low"):
    m = llm_raw(messages, max_tokens, effort)["choices"][0]["message"]
    return (m.get("content") or "").strip()


# ── Vositalar (model "qo'llari") ─────────────────────────────

TOOLS = [
    {"type": "function", "function": {
        "name": "shell", "description": "Bash buyrug'ini ish papkasida ishga tushiradi; stdout+stderr va chiqish kodi qaytadi.",
        "parameters": {"type": "object", "properties": {
            "cmd": {"type": "string", "description": "bash buyrug'i"},
            "timeout": {"type": "integer", "description": "soniya (standart 120, ko'pi 900)"}}, "required": ["cmd"]}}},
    {"type": "function", "function": {
        "name": "read_file", "description": "Ish papkasidagi matnli faylni o'qiydi (boshidan ~6000 belgi).",
        "parameters": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}}},
    {"type": "function", "function": {
        "name": "write_file", "description": "Ish papkasida faylni yozadi (mavjud bo'lsa ustiga).",
        "parameters": {"type": "object", "properties": {"path": {"type": "string"}, "content": {"type": "string"}},
                       "required": ["path", "content"]}}},
    {"type": "function", "function": {
        "name": "get_attachment", "description": "Foydalanuvchi yuborgan faylni (xabardagi #N raqami) ish papkasiga yuklaydi; yo'li qaytadi.",
        "parameters": {"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id"]}}},
    {"type": "function", "function": {
        "name": "send_file", "description": "Ish papkasidagi faylni foydalanuvchiga Telegram'da yuboradi.",
        "parameters": {"type": "object", "properties": {"path": {"type": "string"}, "caption": {"type": "string"}},
                       "required": ["path"]}}},
]


def _safe_path(path):
    """Ish papkasidan tashqariga chiqmaydigan to'liq yo'l."""
    full = os.path.realpath(os.path.join(WORKDIR, str(path)))
    if full != WORKDIR and not full.startswith(WORKDIR + os.sep):
        raise ValueError("yo'l ish papkasidan tashqarida")
    return full


def _as_agent(argv, stdin=None, timeout=120):
    """Buyruq maxfiy kalitlarsiz, alohida foydalanuvchi sifatida (RUN_KEY va tokenlar unga ko'rinmaydi)."""
    return subprocess.run(
        ["sudo", "-n", "-u", AGENT_USER, "-H", "env", "-i", f"HOME=/home/{AGENT_USER}", "LANG=C.UTF-8",
         f"PATH=/home/{AGENT_USER}/.local/bin:/usr/local/bin:/usr/bin:/bin", *argv],
        input=stdin, capture_output=True, timeout=timeout)


def _clip(text, n=TOOL_OUT):
    text = text.strip()
    if len(text) <= n:
        return text
    return text[: n // 2] + f"\n…[{len(text) - n} belgi qisqartirildi]…\n" + text[-n // 2:]


def tool_shell(a, msg):
    t = max(5, min(int(a.get("timeout") or 120), 900))
    r = _as_agent(["timeout", "-k", "5", str(t), "bash", "-c", f"cd {WORKDIR} && {a['cmd']}"], timeout=t + 20)
    out = (r.stdout + r.stderr).decode(errors="replace")
    return _clip(out) + f"\n[chiqish kodi: {r.returncode}]" + (" (vaqt tugadi)" if r.returncode == 124 else "")


def tool_read_file(a, msg):
    full = _safe_path(a["path"])
    r = _as_agent(["head", "-c", "6000", full])
    if r.returncode:
        return "xato: " + r.stderr.decode(errors="replace")[:300]
    return r.stdout.decode(errors="replace")


def tool_write_file(a, msg):
    full = _safe_path(a["path"])
    _as_agent(["mkdir", "-p", os.path.dirname(full)])
    r = _as_agent(["tee", full], stdin=str(a.get("content", "")).encode())
    if r.returncode:
        return "xato: " + r.stderr.decode(errors="replace")[:300]
    return f"yozildi: {a['path']} ({len(str(a.get('content', '')))} belgi)"


def tool_get_attachment(a, msg):
    base = API
    req = urllib.request.Request(base + "/ai/getfile", data=json.dumps({"id": int(a["id"])}).encode(), method="POST",
                                 headers={"X-Run-Key": RUN_KEY, "Content-Type": "application/json", "User-Agent": "albot-run"})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            name = urllib.parse.unquote(r.headers.get("X-Name") or f"file_{a['id']}")
            name = re.sub(r"[^\w.\- ]+", "_", name)[:100] or f"file_{a['id']}"
            data = r.read()
    except urllib.error.HTTPError as e:
        return "xato: " + e.read().decode(errors="replace")[:300]
    dst = os.path.join(WORKDIR, name)
    _as_agent(["tee", dst], stdin=data, timeout=120)
    return f"yuklandi: {name} ({len(data)} bayt) — ish papkasida"


def tool_send_file(a, msg):
    full = _safe_path(a["path"])
    r = _as_agent(["cat", full], timeout=120)
    if r.returncode or not r.stdout:
        return "xato: fayl topilmadi yoki bo'sh"
    req = urllib.request.Request(
        API + "/ai/file", data=r.stdout, method="POST",
        headers={"X-Run-Key": RUN_KEY, "X-Chat": str(msg["chat"]), "X-Name": urllib.parse.quote(os.path.basename(full)),
                 "X-Caption": urllib.parse.quote(str(a.get("caption") or "")), "Content-Type": "application/octet-stream",
                 "User-Agent": "albot-run"})
    try:
        with urllib.request.urlopen(req, timeout=300) as resp:
            return "yuborildi" if json.loads(resp.read() or b"{}").get("ok") else "yuborilmadi"
    except urllib.error.HTTPError as e:
        return "xato: " + e.read().decode(errors="replace")[:300]


TOOL_FUNCS = {"shell": tool_shell, "read_file": tool_read_file, "write_file": tool_write_file,
              "get_attachment": tool_get_attachment, "send_file": tool_send_file}


def run_tool(call, msg):
    fn = call.get("function", {})
    name = fn.get("name", "")
    try:
        args = fn.get("arguments") or {}
        if isinstance(args, str):
            args = json.loads(args or "{}")
        if name not in TOOL_FUNCS:
            return f"xato: '{name}' degan vosita yo'q"
        return TOOL_FUNCS[name](args, msg)
    except subprocess.TimeoutExpired:
        return "xato: buyruq vaqti tugadi"
    except Exception as e:  # noqa: BLE001
        return f"xato: {type(e).__name__}: {e}"


def agent_answer(msg, summary, history):
    """Model vositalarni chaqirib ishlaydi; oxirida yakuniy matn qaytadi."""
    msgs = build_messages(summary, history)
    t_end = min(time.time() + AGENT_MAX_MIN * 60, T0 + MAX_MIN * 60 - 120)
    steps = 0
    failed = {}
    while True:
        use_tools = steps < MAX_STEPS and time.time() < t_end and sum(len(str(m.get("content", ""))) for m in msgs) < 42000
        resp = llm_raw(msgs, tools=TOOLS if use_tools else None)
        m = resp["choices"][0]["message"]
        calls = m.get("tool_calls") or []
        if not calls or not use_tools:
            ans = (m.get("content") or "").strip()
            if not use_tools and steps:
                ans = (ans + "\n\n(Qadamlar/vaqt/kontekst chegarasi tugadi — shu yerda to'xtadim.)").strip()
            return ans
        msgs.append({"role": "assistant", "content": m.get("content") or "", "tool_calls": calls})
        for c in calls:
            fn = c.get("function", {})
            raw = fn.get("arguments")
            shown = raw if isinstance(raw, str) else json.dumps(raw or {}, ensure_ascii=False)
            log(f"🔧 {fn.get('name')} {shown[:200]}")
            try:
                call("/ai/say", {"chat": msg["chat"], "text": f"🔧 {fn.get('name')}: {shown[:300]}"}, timeout=20)
            except Exception:  # noqa: BLE001
                pass
            key = (fn.get("name"), shown)
            if failed.get(key, 0) >= 2:
                res = "xato: bu chaqiruv 2 marta xato bergan — qayta urinma; boshqa yo'l tut yoki foydalanuvchiga sababini ayt"
            else:
                res = run_tool(c, msg)
                if res.startswith("xato"):
                    failed[key] = failed.get(key, 0) + 1
            log(f"   → {res[:200]!r}")
            msgs.append({"role": "tool", "tool_call_id": c.get("id", ""), "content": res})
        steps += 1


def build_messages(summary, history):
    sysmsg = SYSTEM
    if summary:
        sysmsg += "\n\nAvvalgi suhbat xulosasi:\n" + summary
    # Kontekstga sig'ishi uchun eng eski xabarlar tashlanadi (belgi hisobida).
    kept, total = [], 0
    for m in reversed(history):
        total += len(m["t"])
        if total > HIST_CHARS and kept:
            break
        kept.append(m)
    kept.reverse()
    msgs = [{"role": "system", "content": sysmsg}]
    for m in kept:
        role = "user" if m["r"] == 0 else "assistant"
        if msgs[-1]["role"] == role and role != "system":
            msgs[-1]["content"] += "\n" + m["t"]
        else:
            msgs.append({"role": role, "content": m["t"]})
    return msgs


def maybe_summarize(chat, summary, history):
    total = sum(len(m["t"]) for m in history)
    if total <= HIST_CHARS:
        return
    # Oxirgi TAIL_CHARS belgilik qismi qoladi, undan eskisi xulosaga aylanadi.
    tail, acc = 0, 0
    for i in range(len(history) - 1, -1, -1):
        acc += len(history[i]["t"])
        if acc >= TAIL_CHARS:
            tail = i
            break
    old = history[:tail]
    if not old:
        return
    text = "\n".join(("Foydalanuvchi: " if m["r"] == 0 else "Model: ") + m["t"] for m in old)
    prompt = ("Quyidagi suhbatni keyingi javoblar uchun kerakli barcha muhim faktlar, qarorlar, kod nomlari va "
              "foydalanuvchi talablarini saqlagan holda 1500 belgigacha qisqa xulosaga aylantir.\n\n")
    if summary:
        prompt += "Avvalgi xulosa:\n" + summary + "\n\n"
    out = llm([{"role": "user", "content": prompt + text[-40000:]}], max_tokens=700, effort="low")
    if out:
        call("/ai/summary", {"chat": chat, "summary": out[:3000], "upto": old[-1]["id"]})
        log(f"tarix siqildi: {len(old)} xabar -> {len(out)} belgi")


def typing_loop(chat, stop):
    while not stop.wait(4):
        try:
            call("/ai/typing", {"chat": chat}, timeout=15)
        except Exception:
            pass


def handle(msg, summary, history):
    stop = threading.Event()
    threading.Thread(target=typing_loop, args=(msg["chat"], stop), daemon=True).start()
    try:
        t = time.time()
        answer = agent_answer(msg, summary, history)
        log(f"javob {len(answer)} belgi, {time.time() - t:.0f}s")
        call("/ai/reply", {"id": msg["id"], "text": answer})
    except Exception as e:
        log("xato:", e)
        try:
            call("/ai/fail", {"id": msg["id"], "error": str(e)[:300]})
        except Exception:
            pass
        return
    finally:
        stop.set()
    try:
        maybe_summarize(msg["chat"], summary, history)
    except Exception as e:
        log("siqish xato:", e)


def main():
    srv = subprocess.Popen(
        [LLAMA, "-m", MODEL, "--host", "127.0.0.1", "--port", str(PORT), "-c", str(CTX), "-t", str(os.cpu_count() or 4),
         "--jinja"],
        stdout=open("/tmp/llama.log", "w"), stderr=subprocess.STDOUT,
    )
    try:
        log("model yuklanmoqda…")
        for _ in range(120):
            if srv.poll() is not None:
                print(open("/tmp/llama.log").read()[-3000:])
                raise SystemExit("llama-server to'xtadi")
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{PORT}/health", timeout=3).read()
                break
            except Exception:
                time.sleep(5)
        else:
            raise SystemExit("model 10 daqiqada yuklanmadi")
        log("model tayyor")
        while True:
            if time.time() - T0 > MAX_MIN * 60:
                log("vaqt chegarasi — tugatilmoqda")
                break
            try:
                r = call("/ai/poll", {}, timeout=30)
            except Exception as e:
                log("poll xato:", e)
                time.sleep(5)
                continue
            msg = r.get("msg")
            if not msg:
                log("navbat bo'sh — tugatilmoqda")
                break
            log(f"xabar #{msg['id']}: {msg['text'][:60]!r}")
            handle(msg, r.get("summary", ""), r.get("history", []))
        call("/ai/done", {"ok": True}, timeout=30)
    finally:
        srv.terminate()


if __name__ == "__main__":
    main()
