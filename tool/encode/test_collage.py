"""Sinov: `python3 tool/encode/test_collage.py` — `make_collage` (run.py ichidan ajratib olinadi) 16:9 (1920x1080) JPEG chiqarishi kerak."""
import ast, shutil, subprocess, sys, tempfile
from pathlib import Path

src = Path(__file__).with_name("run.py").read_text()
node = next(n for n in ast.parse(src).body if isinstance(n, ast.FunctionDef) and n.name == "make_collage")
ns = {"Path": Path, "subprocess": subprocess, "shutil": shutil}
exec(compile(ast.Module([node], []), "run.py", "exec"), ns)

d = Path(tempfile.mkdtemp())
v = d / "t.mp4"
subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=25", "-f", "lavfi", "-i", "sine=frequency=440",
                "-t", "60", "-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac", str(v)], check=True)
data = ns["make_collage"](v, 60.0)
(d / "c.jpg").write_bytes(data)
w, h = subprocess.check_output(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0:s=x", str(d / "c.jpg")]).decode().strip().split("x")
assert (int(w), int(h)) == (1920, 1080), (w, h)
assert len(data) > 20000, len(data)
print("test_collage: OK", w, h, len(data), "bayt")
