"""Sinov: `build_each` — har yuborilgan fayl o'zi kabi tur bilan qaytadi (rasm/PDF/ZIP-CBZ)."""
import ast, re, shutil, tempfile, zipfile
from pathlib import Path
from collections import OrderedDict

src = Path(__file__).with_name("translate.py").read_text()
tree = ast.parse(src)
body = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in ("build_each", "build_output")]
ns = {"re": re, "shutil": shutil, "zipfile": zipfile, "Path": Path, "OrderedDict": OrderedDict}
exec(compile(ast.Module(body, []), "translate.py", "exec"), ns)

from PIL import Image
d = Path(tempfile.mkdtemp())
imgs = []
for i in range(5):
    p = d / f"{i}.jpg"
    Image.new("RGB", (40, 60), (i * 40, 10, 10)).save(p)
    imgs.append(p)
# 1-fayl: rasm, 2-fayl: PDF (2 sahifa), 3-fayl: CBZ (2 sahifa)
owners = [0, 1, 1, 2, 2]
pages = [{"n": "a.png", "k": "img"}, {"n": "bob1.pdf", "k": "pdf"}, {"n": "bob2.cbz", "k": "zip"}]
res = ns["build_each"](imgs, owners, pages, d / "each")
names = [n for _, n in res]
assert names == ["a_uz.jpg", "bob1_uz.pdf", "bob2_uz.cbz"], names
assert all(f.exists() and f.stat().st_size > 0 for f, _ in res)
assert len(zipfile.ZipFile(res[2][0]).namelist()) == 2
assert res[1][0].read_bytes()[:4] == b"%PDF"
print("test_each: OK")
