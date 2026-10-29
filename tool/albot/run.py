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
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request

API = os.environ["API"].rstrip("/")
RUN_KEY = "".join(os.environ["RUN_KEY"].split())
MODEL = os.environ["MODEL_PATH"]
LLAMA = os.environ["LLAMA_SERVER"]
PORT = 8080
MAX_MIN = float(os.environ.get("MAX_MIN", "45"))
CTX = 16384
HIST_CHARS = 24000   # shundan oshsa tarix siqiladi
TAIL_CHARS = 6000    # siqishdan keyin shuncha yangi xabar qoladi
MAX_TOKENS = 1500

SYSTEM = (
    "Sen foydalanuvchining shaxsiy yordamchisisan. O'zbek tilida (agar foydalanuvchi boshqa tilda yozmasa) aniq, qisqa va "
    "to'g'ri javob ber. Kod so'ralsa — ishlaydigan kod va qisqa izoh ber. Bilmasang, bilmasligingni ayt."
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


def llm(messages, max_tokens=MAX_TOKENS, effort="low"):
    body = {
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": 0.7,
        "chat_template_kwargs": {"reasoning_effort": effort},
    }
    req = urllib.request.Request(
        f"http://127.0.0.1:{PORT}/v1/chat/completions",
        data=json.dumps(body).encode(), method="POST", headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=1500) as r:
        j = json.loads(r.read().decode())
    m = j["choices"][0]["message"]
    return (m.get("content") or "").strip()


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
        answer = llm(build_messages(summary, history))
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
         "--jinja", "--no-webui"],
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
