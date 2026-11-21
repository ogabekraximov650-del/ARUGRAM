"""Sinov: `python3 tool/encode/test_collage.py` — `make_frames` (run.py ichidan ajratib olinadi) 10 ta TO'LIQ o'lchamli (1920x1080, 16:9) JPEG kadr qaytarishi,
kadrlar videoning 20%..80% oralig'idan olinishi va `pack_frames` formati (4 bayt uzunlik + kadr) to'g'ri ekanini tekshiradi."""
import ast, shutil, subprocess, sys, tempfile
from pathlib import Path

src = Path(__file__).with_name("run.py").read_text()
want = ("make_frames", "pack_frames")
nodes = [n for n in ast.parse(src).body if (isinstance(n, ast.FunctionDef) and n.name in want)
         or (isinstance(n, ast.Assign) and "FRAMES_" in ast.unparse(n.targets[0]))]
ns = {"Path": Path, "subprocess": subprocess, "shutil": shutil}
exec(compile(ast.Module(nodes, []), "run.py", "exec"), ns)

d = Path(tempfile.mkdtemp())
v = d / "t.mp4"
# Har soniyada rangi o'zgaradigan sinov videosi + sekundomer (vaqt kadrda ko'rinadi), 100 s.
subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=25", "-f", "lavfi", "-i", "sine=frequency=440",
                "-t", "100", "-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac", str(v)], check=True)
frames = ns["make_frames"](v, 100.0)
assert len(frames) == 10, len(frames)
for i, f in enumerate(frames):
    (d / f"f{i}.jpg").write_bytes(f)
    w, h = subprocess.check_output(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0:s=x",
                                    str(d / f"f{i}.jpg")]).decode().strip().split("x")
    assert (int(w), int(h)) == (1920, 1080), (i, w, h)
    assert f[:2] == b"\xff\xd8" and len(f) > 20000, (i, len(f))
# Birinchi va oxirgi kadr 20%..80% (20 s..80 s) oralig'idan: bo'lak oralig'i 6 s, demak 1-kadr >= 20 s, oxirgi <= 80 s.
# Sinov: har kadrning o'rtacha yorqinligi har xil emas, shuning uchun vaqtni o'rtacha rang bilan emas, bo'lak tartibi bilan tekshiramiz (hammasi har xil).
assert len({f for f in frames}) == 10, "kadrlar takrorlanmoqda"
packed = ns["pack_frames"](frames)
pos, got = 0, []
while pos < len(packed):
    n = int.from_bytes(packed[pos:pos + 4], "big")
    got.append(packed[pos + 4:pos + 4 + n])
    pos += 4 + n
assert got == frames and pos == len(packed)
# 20%..80% oralig'i: kadr yorqinligi vaqtga bog'liq video (kulrang = 2.55 * soniya, 100 s) — birinchi kadr >= 20 s, oxirgisi <= 80 s ekanini tekshiramiz.
g = d / "g.mp4"
subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=1280x720:r=10,geq=lum='min(255,2.55*T)':cb=128:cr=128",
                "-t", "100", "-c:v", "libx264", "-preset", "ultrafast", str(g)], check=True)
tf = ns["make_frames"](g, 100.0)


def gray(b):
    p = d / "x.jpg"
    p.write_bytes(b)
    out = subprocess.check_output(["ffmpeg", "-v", "error", "-i", str(p), "-vf", "scale=1:1", "-f", "rawvideo", "-pix_fmt", "gray", "-"])
    return out[0] / 2.55    # taxminiy soniya


def ref(t):   # aynan t-soniyadagi kadr (kulrang qiymatini o'lchash usuli bilan bir xil o'zgartirish: rang oralig'i/JPEG farqini hisobga olish uchun)
    p = d / "r.jpg"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", str(t), "-i", str(g), "-frames:v", "1", "-q:v", "2", str(p)], check=True)
    return gray(p.read_bytes())


lo, hi = ref(20), ref(80)
secs = [gray(b) for b in tf]
assert min(secs) >= lo - 0.5, (lo, secs)
assert max(secs) <= hi + 0.5, (hi, secs)
assert secs == sorted(secs), secs   # tartib saqlanadi
print("kadr vaqtlari (s):", [round(x) for x in secs])
print("test_collage: OK", len(frames), "kadr,", sum(len(f) for f in frames) // 1024, "KB")
