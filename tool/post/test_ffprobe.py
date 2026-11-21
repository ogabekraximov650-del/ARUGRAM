"""Sinov: `python3 tool/post/test_ffprobe.py` — post.py dagi `ffprobe()` (video kengligi/balandligi/davomiyligi) HOZIRGI ffmpeg versiyasida to'g'ri qiymat qaytarishi kerak.
Sabab: Telegram'ga `width`/`height` berilmasa video 320x320 kvadrat bo'lib ko'rinadi va to'liq ekran tugmasi chiqmaydi (kanal xabari 3178). Versiya chop etiladi."""
import ast, subprocess, sys, tempfile
from pathlib import Path

print(subprocess.run(["ffprobe", "-version"], capture_output=True, text=True).stdout.splitlines()[0])
node = next(n for n in ast.parse(Path(__file__).with_name("post.py").read_text()).body if isinstance(n, ast.FunctionDef) and n.name == "ffprobe")
ns = {"subprocess": subprocess}
exec(compile(ast.Module([node], []), "post.py", "exec"), ns)
fp = ns["ffprobe"]

d = Path(tempfile.mkdtemp())
cases = {
    "oddiy": ["-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=24", "-f", "lavfi", "-i", "sine=frequency=440", "-t", "3", "-c:v", "libx264", "-c:a", "aac"],
    "hevc": ["-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=25", "-f", "lavfi", "-i", "sine=frequency=440", "-t", "3", "-c:v", "libx265", "-tag:v", "hvc1", "-c:a", "aac"],
}
bad = 0
for name, args in cases.items():
    p = d / f"{name}.mp4"
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error"] + args + ["-movflags", "+faststart", str(p)], check=True)
    w, h, dur = fp(p, "width", "v:0"), fp(p, "height", "v:0"), fp(p, "duration")
    raw = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width", "-of", "csv=p=0", str(p)], capture_output=True, text=True)
    print(f"{name}: w={w} h={h} dur={dur} | xom chiqish: {raw.stdout!r} xato: {raw.stderr[:100]!r}")
    want = (1280, 720) if name == "oddiy" else (1920, 1080)
    if (w, h) != want or dur != 3:
        bad += 1
if bad:
    sys.exit("ffprobe() noto'g'ri qiymat qaytardi — Telegram'ga video o'lchami berilmaydi")
print("test_ffprobe: OK")
