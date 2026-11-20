"""Sinov: `python3 tool/translate/test_manual.py` — qo'lda tarjima: `sahifa.raqam#` matnini tahlil qilish va pufakchalarni JSON'ga saqlash/o'qish."""
import ast, json, re
from pathlib import Path

src = Path(__file__).with_name("translate.py").read_text()
tree = ast.parse(src)
want_f = {"parse_manual", "serialize_groups", "load_groups", "keep_group"}
body = [n for n in tree.body if (isinstance(n, ast.FunctionDef) and n.name in want_f)
        or (isinstance(n, ast.Assign) and any(getattr(t, "id", "") in ("MARK_RE", "JUNK_WORDS") for t in n.targets))]
ns = {"re": re, "json": json}
exec(compile(ast.Module(body, []), "translate.py", "exec"), ns)

text = "1.1# salom\n1.2# qayerga\nketyapsan?\n2.1 bilmayman\n  3.10 :  oxirgi\nmarker yo'q qator"
m = ns["parse_manual"](text)
assert m[(1, 1)] == "salom", m
assert m[(1, 2)] == "qayerga ketyapsan?", m
assert m[(2, 1)] == "bilmayman", m
assert m[(3, 10)] == "oxirgi marker yo'q qator", m
groups = [[{"box": (1, 2, 30, 40), "boxes": [(1, 2, 10, 20), (12, 22, 30, 40)], "text": "x"}], []]
back = ns["load_groups"](json.dumps(ns["serialize_groups"](groups)))
assert back[0][0]["box"] == (1, 2, 30, 40) and back[0][0]["boxes"][1] == (12, 22, 30, 40) and back[1] == [], back
kg = ns["keep_group"]
assert not kg({"text": "26 {o", "conf": 0.6}), "shovqin saqlanib qoldi"
assert not kg({"text": "Merakiscans.com Translator YuJJun", "conf": 0.9}), "kredit saqlanib qoldi"
assert not kg({"text": "HAAH", "conf": 0.1}), "past ishonch"
assert kg({"text": "Haah", "conf": 0.9}) and kg({"text": "E-RANK HUNTER;", "conf": 0.8}) and kg({"text": "", "conf": 0.0})
print("test_manual: OK")
