"""yt-dlp Telegram boti (webhook, Ample Computer). Fayl cloud Bot API orqali <=48 MB bo'laklarda yuboriladi.
UZ: faqat webhook; sessiya/api_id kerak emas. EN: webhook-only, no sessions. RU: только webhook, без сессий."""
import asyncio, glob, hmac, json, logging, os, re, shutil, subprocess, sys, tempfile
from aiohttp import web
from tg import Tg, TgError

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(message)s")
log = logging.getLogger("ytbot")
TOKEN = os.environ["BOT_TOKEN"]
ADMIN = int(os.environ.get("ADMIN_ID", "0"))
SECRET = os.environ.get("WEBHOOK_SECRET", "")
PORT = int(os.environ.get("PORT", "8080"))
LIMIT = 48 * 1024 * 1024
tg = Tg(TOKEN)
queue: asyncio.Queue = asyncio.Queue()
URL_RE = re.compile(r"https?://\S+")

try:
    import imageio_ffmpeg
    FFMPEG = imageio_ffmpeg.get_ffmpeg_exe()
except Exception:
    FFMPEG = shutil.which("ffmpeg") or "ffmpeg"
AWAKE = shutil.which("ample-awake")


def run(cmd, timeout=3600):
    if AWAKE:
        cmd = [AWAKE, "--"] + cmd
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def split(path, outdir):
    size = os.path.getsize(path)
    if size <= LIMIT:
        return [path]
    r = subprocess.run([FFMPEG, "-i", path], capture_output=True, text=True)
    m = re.search(r"Duration: (\d+):(\d+):([\d.]+)", r.stderr)
    dur = int(m[1]) * 3600 + int(m[2]) * 60 + float(m[3]) if m else 0
    if not dur:
        raise RuntimeError("davomiylik aniqlanmadi, bo'lib bo'lmadi")
    seg = max(10, int(dur * LIMIT / size * 0.85))
    ext = os.path.splitext(path)[1]
    pat = os.path.join(outdir, f"part%03d{ext}")
    p = subprocess.run([FFMPEG, "-y", "-i", path, "-map", "0:v:0", "-map", "0:a?", "-sn", "-dn", "-c", "copy", "-f", "segment", "-segment_format", "mp4",
                        "-segment_time", str(seg), "-reset_timestamps", "1", pat], capture_output=True, text=True)
    parts = sorted(glob.glob(os.path.join(outdir, "part*" + ext)))
    if p.returncode or not parts:
        raise RuntimeError("bo'lishda xato: " + " | ".join([l for l in p.stderr.splitlines() if "rror" in l or "nvalid" in l or "annot" in l][:3])[:400] or p.stderr[:300])
    return parts


async def job(chat, url, audio):
    st = await tg.send_message(chat, "⏳ Yuklanmoqda…")
    mid = st["message_id"]
    tmp = tempfile.mkdtemp(prefix="yt")
    try:
        out = os.path.join(tmp, "%(title).60B.%(ext)s")
        cmd = [os.path.join(os.path.dirname(sys.executable), "yt-dlp"), "--no-playlist", "--no-warnings", "-o", out, "--ffmpeg-location", FFMPEG,
               "--print", "after_move:filepath"]
        ck = os.environ.get("COOKIES", "/srv/ytbot/cookies.txt")
        if os.path.isfile(ck):
            cmd += ["--cookies", ck]
        if audio:
            cmd += ["-x", "--audio-format", "mp3", "-f", "ba/b"]
        else:
            cmd += ["-S", "res:720,ext:mp4:m4a", "--merge-output-format", "mp4"]
        r = await asyncio.to_thread(run, cmd + [url])
        files = [l for l in r.stdout.splitlines() if os.path.isfile(l.strip())]
        if r.returncode or not files:
            raise RuntimeError((r.stderr.strip().splitlines() or ["yuklab bo'lmadi"])[-1][:300])
        f = files[-1].strip()
        title = os.path.splitext(os.path.basename(f))[0]
        sub = os.path.join(tmp, "p")
        os.makedirs(sub)
        parts = await asyncio.to_thread(split, f, sub)
        for i, p in enumerate(parts, 1):
            await tg.edit_message(chat, mid, f"📤 Yuborilmoqda {i}/{len(parts)}…")
            cap = title + (f" ({i}/{len(parts)})" if len(parts) > 1 else "")
            fl = {"audio" if audio else "video": (p, os.path.basename(p))}
            fields = {"chat_id": chat, "caption": cap[:1000]}
            if not audio:
                fields["supports_streaming"] = "true"
            await tg.upload("sendAudio" if audio else "sendVideo", fields, fl)
        await tg.delete_message(chat, mid)
    except Exception as e:
        log.warning("xato: %s", e)
        await tg.edit_message(chat, mid, f"❌ {str(e)[:500]}")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


async def worker():
    while True:
        chat, url, audio = await queue.get()
        try:
            await job(chat, url, audio)
        except Exception as e:
            log.warning("job: %s", e)


async def handle_update(u):
    m = u.get("message") or {}
    chat = (m.get("chat") or {}).get("id")
    if not chat or (m.get("from") or {}).get("id") != ADMIN:
        return
    text = m.get("text") or ""
    d = m.get("document")
    if d and (d.get("file_name") or "").lower().endswith(".txt"):
        try:
            fi = await tg.call("getFile", file_id=d["file_id"])
            ses = await tg.session()
            async with ses.get(f"https://api.telegram.org/file/bot{TOKEN}/{fi['file_path']}") as r:
                data = await r.read()
            if b"youtube.com" not in data and b".google.com" not in data:
                await tg.send_message(chat, "❌ Bu youtube cookies.txt emas")
            else:
                open("/srv/ytbot/cookies.txt", "wb").write(data)
                await tg.send_message(chat, "✅ cookies.txt saqlandi. Endi YouTube havolasini yuboring.")
        except Exception as e:
            await tg.send_message(chat, f"❌ {e}")
        return
    if text.startswith("/cookies"):
        rows = []
        for ln in text.splitlines()[1:]:
            ln = ln.strip()
            k, sep, v = ln.partition("=")
            if not sep:
                k, _, v = ln.partition(" ")
            k, v = k.strip(), v.strip()
            if k and v:
                for dom in (".youtube.com", ".google.com"):
                    rows.append("\t".join([dom, "TRUE", "/", "TRUE", "2000000000", k, v]))
        if not rows:
            await tg.send_message(chat, "Format:\n<code>/cookies\nSID=...\nHSID=...\nSSID=...</code>")
        else:
            names = {r.split("\t")[5] for r in rows}
            keep = []
            try:
                for l in open("/srv/ytbot/cookies.txt").read().splitlines():
                    c = l.split("\t")
                    if not l.startswith("#") and len(c) == 7 and c[5] not in names:
                        keep.append(l)
            except OSError:
                pass
            with open("/srv/ytbot/cookies.txt", "w") as f:
                f.write("# Netscape HTTP Cookie File\n" + "\n".join(keep + rows) + "\n")
            await tg.delete_message(chat, m.get("message_id"))
            await tg.send_message(chat, f"✅ {len(rows) // 2} ta cookie saqlandi (xabaringiz o'chirildi). YouTube havolasini yuboring.")
        return
    mt = URL_RE.search(text)
    if mt:
        audio = text.lower().startswith(("/a ", "/audio", "a "))
        queue.put_nowait((chat, mt.group(0), audio))
        if queue.qsize() > 1:
            await tg.send_message(chat, f"🕓 Navbatda: {queue.qsize()}")
    else:
        await tg.send_message(chat, "Havola yuboring (YouTube, Instagram, TikTok …) — video keladi.\n"
                              "Faqat ovoz: <code>/a havola</code>\nKatta fayl 48 MB lik bo'laklarga bo'linadi.")


async def hook(req):
    if SECRET and not hmac.compare_digest(req.headers.get("X-Telegram-Bot-Api-Secret-Token", ""), SECRET):
        return web.Response(status=403)
    try:
        asyncio.create_task(handle_update(await req.json()))
    except Exception as e:
        log.warning("hook: %s", e)
    return web.Response(text="ok")


async def health(_):
    return web.json_response({"ok": True, "queue": queue.qsize()})


async def on_start(app):
    app["w"] = asyncio.create_task(worker())


app = web.Application()
app.router.add_post("/webhook", hook)
app.router.add_get("/", health)
app.on_startup.append(on_start)
if __name__ == "__main__":
    web.run_app(app, port=PORT)
