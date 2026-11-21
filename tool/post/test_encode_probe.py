"""Sinov: HAQIQIY `encode.sh` (ICC profilli cover rasm bilan) chiqargan videoda post.py `ffprobe()` kenglik/balandlik/davomiylikni to'g'ri o'qiydimi?
Xabar 3178 (Telegram'da 320x320 deb yozilgan) shu yo'lda hosil bo'lgan bo'lishi mumkin: yangi ffmpeg (8.x) videoga ICC profil yozadi. Ko'rinish chop etiladi."""
import ast, os, subprocess, sys, tempfile
from pathlib import Path

here = Path(__file__).parent
node = next(n for n in ast.parse((here / "post.py").read_text()).body if isinstance(n, ast.FunctionDef) and n.name == "ffprobe")
import json
ns = {"subprocess": subprocess, "json": json}
exec(compile(ast.Module([node], []), "post.py", "exec"), ns)
fp = ns["ffprobe"]

from PIL import Image, ImageCms
d = Path(tempfile.mkdtemp())
icc = ImageCms.ImageCmsProfile(ImageCms.createProfile("sRGB")).tobytes()
Image.new("RGB", (1280, 720), (30, 90, 160)).save(d / "cover.png", icc_profile=icc)
Image.new("RGBA", (300, 100), (255, 0, 0, 255)).save(d / "logo.png")
subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=24000/1001", "-f", "lavfi", "-i", "sine=frequency=440",
                "-t", "4", "-c:v", "libx264", "-c:a", "aac", str(d / "src.mp4")], check=True)
env = dict(os.environ, H265_PRESET="ultrafast")
r = subprocess.run(["bash", str(here / "encode.sh"), str(d / "src.mp4"), str(d / "cover.png"), str(d / "logo.png"), str(d / "out.mp4")], env=env, capture_output=True, text=True)
if r.returncode != 0:
    sys.exit("encode.sh yiqildi:\n" + r.stdout[-600:] + r.stderr[-600:])
out = d / "out.mp4"
w, h, dur = fp(out, "width", "v:0"), fp(out, "height", "v:0"), fp(out, "duration")
info = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "stream=index,codec_type,codec_name,width,height,r_frame_rate:stream_side_data", "-of", "default=nw=1", str(out)],
                      capture_output=True, text=True).stdout
print("ffprobe (encode.sh chiqishi):\n" + info)
print(f"post.py ffprobe(): w={w} h={h} dur={dur}")
if (w, h) != (1280, 720) or not dur:
    sys.exit("encode.sh chiqishida video o'lchami o'qilmadi — Telegram'ga w/h berilmaydi (320x320)")
print("test_encode_probe: OK")
