"""To'plam fayli va rasm yengillashtirish testlari.

Ishga tushirish:  cd tool/packs && python3 -m unittest -v test_arupack
"""

import io
import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image

import arunorm
import arupack
import run as packs_run


def png(w=64, h=64, color=(255, 0, 0, 255)) -> bytes:
    b = io.BytesIO()
    Image.new("RGBA", (w, h), color).save(b, "PNG")
    return b.getvalue()


def gif(frames=6, w=64, h=64, ms=100) -> bytes:
    imgs = [Image.new("RGB", (w, h), (i * 30 % 255, 80, 200)) for i in range(frames)]
    b = io.BytesIO()
    imgs[0].save(b, "GIF", save_all=True, append_images=imgs[1:], duration=ms, loop=0)
    return b.getvalue()


class FormatTests(unittest.TestCase):
    def test_roundtrip_va_shifrlash(self):
        with tempfile.TemporaryDirectory() as d:
            work = Path(d)
            p = arupack.Pack(77, "sticker", "Mushuklar")
            a = p.add(b"AAAA", b"tA", False, 512, 512, "😀")
            b = p.add(b"BBBBBB", b"tB", True, 256, 300, "🐱")
            self.assertEqual((a, b), (1, 2))
            p.version = 3
            sealed, key, size = arupack.seal(p, work)
            enc = sealed.read_bytes()
            self.assertEqual(len(enc), size)
            # Shifrlangan bayt ochiq matn emas.
            self.assertNotIn(b"ARUP", enc[:4])
            plain = arupack.ctr_bytes(enc, key)
            h = arupack.read_header_bytes(plain)
            self.assertEqual(h["id"], 77)
            self.assertEqual(h["ver"], 3)
            self.assertEqual(h["next"], 3)
            base = arupack.PREAMBLE + arupack.parse_preamble(plain[:16])
            it = h["items"][1]
            self.assertEqual(plain[base + it["o"]: base + it["o"] + it["l"]], b"BBBBBB")
            self.assertEqual(plain[base + it["to"]: base + it["to"] + it["tl"]], b"tB")
            self.assertEqual(it["a"], 1)

            # Fayl o'rtasidan (offset bilan) ochish — ilova shunday o'qiydi.
            off = base + it["o"]
            piece = arupack.ctr_bytes(enc[off: off + it["l"]], key, offset=off)
            self.assertEqual(piece, b"BBBBBB")

    def test_qayta_yozish_yangi_kalit_va_id_qaytmaydi(self):
        with tempfile.TemporaryDirectory() as d:
            work = Path(d)
            p = arupack.Pack(5, "emoji", "E")
            p.add(b"1", b"t", False, 128, 128, "😀")
            p.add(b"2", b"t", False, 128, 128, "😁")
            sealed, key, _ = arupack.seal(p, work)
            plain = work / "old.plain"
            arupack.ctr_file(sealed, plain, key)
            old = arupack.read_plain(plain)
            self.assertTrue(old.remove(2))
            new_id = old.add(b"3", b"t", False, 128, 128, "😂")
            self.assertEqual(new_id, 3, "o'chirilgan raqam qayta ishlatilmasin")
            old.version = 2
            sealed2, key2, _ = arupack.seal(old, work)
            self.assertNotEqual(key, key2, "har versiyaga yangi kalit")
            plain2 = arupack.ctr_bytes(sealed2.read_bytes(), key2)
            h = arupack.read_header_bytes(plain2)
            self.assertEqual([i["i"] for i in h["items"]], [1, 3])

    def test_buzuq_fayl(self):
        with self.assertRaises(arupack.PackError):
            arupack.parse_preamble(b"XXXX" + b"\0" * 12)
        with self.assertRaises(arupack.PackError):
            arupack.Pack(1, "video", "x")


class NormalizeTests(unittest.TestCase):
    def test_statik_stiker(self):
        r = arunorm.normalize(png(1000, 800), "sticker")
        self.assertFalse(r.animated)
        self.assertLessEqual(max(r.w, r.h), 512)
        self.assertEqual(arunorm.sniff(r.data), "webp")
        self.assertEqual(arunorm.sniff(r.thumb), "webp")
        self.assertLess(len(r.thumb), 40000)

    def test_animatsiya_saqlanadi(self):
        r = arunorm.normalize(gif(8), "gif")
        self.assertTrue(r.animated)
        im = Image.open(io.BytesIO(r.data))
        self.assertGreater(getattr(im, "n_frames", 1), 1)

    def test_tez_kadrlar_tashlanadi(self):
        # 10 ms li kadrlar 100 ms ga aylanadi -> ortiqcha kadr bo'lmaydi.
        r = arunorm.normalize(gif(12, ms=10), "gif")
        im = Image.open(io.BytesIO(r.data))
        self.assertLessEqual(im.n_frames, 12)

    def test_emoji_cheklanmagan_nisbat_va_gifda_rasm(self):
        r0 = arunorm.normalize(png(400, 100), "emoji")  # endi rad etilmaydi
        self.assertLessEqual(max(r0.w, r0.h), 128)
        self.assertFalse(arunorm.normalize(png(200, 200), "gif").animated)  # rasm ham mumkin
        r = arunorm.normalize(png(300, 300), "emoji")
        self.assertLessEqual(max(r.w, r.h), 128)

    def test_rad_etishlar(self):
        with self.assertRaises(arunorm.Rejected):
            arunorm.normalize(b"not an image at all", "sticker")
        with self.assertRaises(arunorm.Rejected):
            arunorm.normalize(png() + b"\0" * (6 * 1024 * 1024), "sticker")
        # Kadrlar soni/uzunlik endi cheklanmaydi.
        self.assertTrue(arunorm.normalize(gif(200, ms=100), "emoji").animated)

    def test_buzuq_png(self):
        with self.assertRaises(arunorm.Rejected):
            arunorm.normalize(png()[:40], "sticker")


def make_video(seconds=3, w=320, h=180, fmt="mp4"):
    import shutil
    import subprocess
    if not shutil.which("ffmpeg"):
        return None
    with tempfile.TemporaryDirectory() as d:
        out = Path(d) / f"v.{fmt}"
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
                        f"testsrc=duration={seconds}:size={w}x{h}:rate=25", str(out)],
                       check=True)
        return out.read_bytes()


class VideoTests(unittest.TestCase):
    def test_video_animatsiyaga_aylanadi(self):
        raw = make_video()
        if raw is None:
            self.skipTest("ffmpeg yo'q")
        self.assertEqual(arunorm.sniff(raw), "mp4")
        r = arunorm.normalize(raw, "gif")
        self.assertTrue(r.animated)
        self.assertLessEqual(len(r.data), arupack.MAX_ITEM)
        self.assertLessEqual(max(r.w, r.h), 640)

    def test_ovozli_video_gifda_mp4_qoladi(self):
        import shutil
        import subprocess
        if not shutil.which("ffmpeg"):
            self.skipTest("ffmpeg yo'q")
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "a.mp4"
            subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
                            "testsrc=duration=2:size=320x180:rate=25", "-f", "lavfi", "-i",
                            "sine=frequency=440:duration=2", "-shortest", str(out)], check=True)
            raw = out.read_bytes()
        r = arunorm.normalize(raw, "gif")
        self.assertTrue(r.video)
        self.assertEqual(arunorm.sniff(r.data), "mp4")
        self.assertLessEqual(len(r.data), arupack.MAX_ITEM)
        # emoji va stikerda ovoz olib tashlanadi (WebP)
        r2 = arunorm.normalize(raw, "sticker")
        self.assertFalse(r2.video)
        # ovozsiz video GIFda ham WebP
        self.assertFalse(arunorm.normalize(make_video(), "gif").video)

    def test_mute_belgisi_ovozni_olib_tashlaydi(self):
        import run
        self.assertTrue(run.mute_of("pki_1_2_3_t0-5000_m.bin"))
        self.assertTrue(run.mute_of("pki_1_2_3_m.bin"))
        self.assertFalse(run.mute_of("pki_1_2_3_t0-5000.bin"))
        self.assertEqual(run.trim_of("pki_1_2_3_t100-5000_m.bin"), (100, 5000))

    def test_moslashmagan_fayl_moslashtiriladi(self):
        import random
        # BMP (sniff bilmaydi) -> Pillow moslashtiradi
        b = io.BytesIO()
        Image.new("RGB", (300, 200), (10, 200, 30)).save(b, "BMP")
        r = arunorm.normalize(b.getvalue(), "sticker")
        self.assertLessEqual(max(r.w, r.h), 384)
        # emoji: rasm ham kvadratga kesiladi
        r = arunorm.normalize(png(400, 100), "emoji")
        self.assertEqual(r.w, r.h)
        # juda og'ir shovqinli animatsiya rad etilmaydi, 5 MB ga sig'diriladi
        rnd = random.Random(1)
        imgs = [Image.frombytes("RGB", (400, 400), bytes(rnd.getrandbits(8) for _ in range(400 * 400 * 3)))
                for _ in range(6)]
        bb = io.BytesIO()
        imgs[0].save(bb, "GIF", save_all=True, append_images=imgs[1:], duration=100, loop=0)
        r = arunorm.normalize(bb.getvalue(), "gif")
        self.assertLessEqual(len(r.data), arupack.MAX_ITEM)

    def test_olchamlar_emoji_stiker_gif(self):
        raw = make_video(seconds=1, w=1280, h=720)
        if raw is None:
            self.skipTest("ffmpeg yo'q")
        e = arunorm.normalize(raw, "emoji")
        s = arunorm.normalize(raw, "sticker")
        g = arunorm.normalize(raw, "gif")
        self.assertLess(max(e.w, e.h), max(s.w, s.h))
        self.assertLess(max(s.w, s.h), max(g.w, g.h))

    def test_kesib_olish_va_emoji_kvadrat(self):
        raw = make_video(seconds=6)
        if raw is None:
            self.skipTest("ffmpeg yo'q")
        r = arunorm.normalize(raw, "emoji", trim=(1000, 3000))
        self.assertEqual(r.w, r.h)
        self.assertLessEqual(r.w, 128)
        im = Image.open(io.BytesIO(r.data))
        # 2 soniya * 15 kadr/s ~ 30 kadr
        self.assertTrue(20 <= im.n_frames <= 32, im.n_frames)

    def test_uzun_video_qabul_qilinadi_qisqa_rad(self):
        raw = make_video(seconds=6)
        if raw is None:
            self.skipTest("ffmpeg yo'q")
        self.assertTrue(arunorm.normalize(raw, "emoji").animated)  # uzunlik cheklanmaydi
        with self.assertRaises(arunorm.Rejected):
            arunorm.normalize(raw, "emoji", trim=(0, 100))  # juda qisqa

    def test_buzuq_video(self):
        with self.assertRaises(arunorm.Rejected):
            arunorm.normalize(b"\x00\x00\x00\x18ftypmp42" + b"junk" * 50, "gif")

    def test_kesim_nomdan_olinadi(self):
        self.assertEqual(packs_run.trim_of("pki_1_2_ab_t1500-4200.bin"), (1500, 4200))
        self.assertIsNone(packs_run.trim_of("pki_1_2_ab.bin"))


class ApplyOpsTests(unittest.TestCase):
    def test_bir_element_xatosi_qolganlarga_tegmaydi(self):
        pack = arupack.Pack(9, "sticker", "T")
        data = {1: png(100, 100), 2: b"garbage", 3: png(50, 50)}

        def fetch(op):
            if op["id"] == 4:
                raise LookupError("fayl kanalda topilmadi")
            return data[op["id"]]

        ops = [
            {"id": 1, "op": "add", "emoji": "😀"},
            {"id": 2, "op": "add", "emoji": "😁"},
            {"id": 3, "op": "add", "emoji": "😂"},
            {"id": 4, "op": "add", "emoji": "🙂"},
        ]
        res = packs_run.apply_ops(pack, ops, fetch)
        self.assertEqual([r["ok"] for r in res], [True, False, True, False])
        self.assertEqual([i.id for i in pack.items], [1, 2])
        self.assertIn("fayl", res[3]["reason"])

    def test_olib_tashlash(self):
        pack = arupack.Pack(9, "gif", "G")
        pack.add(b"x", b"t", False, 1, 1, "")
        res = packs_run.apply_ops(pack, [{"id": 7, "op": "remove", "item_id": 1}], None)
        self.assertTrue(res[0]["ok"])
        self.assertEqual(pack.items, [])
        self.assertEqual(pack.next_id, 2)

    def test_toplam_toldi(self):
        pack = arupack.Pack(9, "sticker", "T")
        orig = arupack.MAX_PACK
        arupack.MAX_PACK = 100
        try:
            res = packs_run.apply_ops(
                pack, [{"id": 1, "op": "add", "emoji": ""}], lambda op: png(64, 64))
        finally:
            arupack.MAX_PACK = orig
        self.assertFalse(res[0]["ok"])
        self.assertIn("to'ldi", res[0]["reason"])


if __name__ == "__main__":
    unittest.main()
