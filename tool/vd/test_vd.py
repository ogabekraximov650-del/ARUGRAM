"""vdlib sinovlari: bo'laklab shifrlash butun fayl CTR shifriga teng; har bo'lak mustaqil ochiladi; eski kalitdan yangisiga o'tkazish."""
import os
import tempfile
import unittest
from pathlib import Path

import vdlib


class T(unittest.TestCase):
    def setUp(self):
        self.c = vdlib.CHUNK
        vdlib.CHUNK = 64  # 16 ga karrali kichik bo'lak

    def tearDown(self):
        vdlib.CHUNK = self.c

    def _file(self, data):
        d = tempfile.mkdtemp()
        p = Path(d) / "f.bin"
        p.write_bytes(data)
        return p

    def test_migratsiya_butun_fayl_ctr_ga_teng(self):
        plain = bytes(range(256)) * 3 + b"xyz"  # 771 bayt, oxirgi bo'lak to'liq emas
        old, new = os.urandom(16), os.urandom(16)
        src = self._file(vdlib.apply(old, 0, plain))
        out = b"".join(b for _, b in vdlib.iter_chunks(src, new, old))
        self.assertEqual(out, vdlib.apply(new, 0, plain))
        self.assertEqual(len(out), len(plain))

    def test_har_bolak_mustaqil_ochiladi(self):
        plain = os.urandom(64 * 5 + 7)
        key = os.urandom(16)
        src = self._file(vdlib.apply(key, 0, plain))
        for n, enc in vdlib.iter_chunks(src, key):  # eski kalitsiz: shunchaki kesadi
            self.assertEqual(vdlib.apply(key, n * 64, enc), plain[n * 64:n * 64 + 64])

    def test_ochiq_fayl_yangi_kalit_bilan(self):
        plain = os.urandom(64 * 3 + 5)
        key = os.urandom(16)
        src = self._file(plain)
        out = b"".join(b for _, b in vdlib.iter_chunks(src, key, None, False, True))
        self.assertEqual(out, vdlib.apply(key, 0, plain))

    def test_bolaklar_soni(self):
        self.assertEqual(vdlib.chunks_of(1), 1)
        self.assertEqual(vdlib.chunks_of(64), 1)
        self.assertEqual(vdlib.chunks_of(65), 2)

    def test_mp4_tekshiruvi(self):
        old, new = os.urandom(16), os.urandom(16)
        good = b"\x00\x00\x00\x18ftypmp42" + b"\x00" * 100
        bad = b"\xde\xad\xbe\xef" * 40
        list(vdlib.iter_chunks(self._file(vdlib.apply(old, 0, good)), new, old, True))
        with self.assertRaises(RuntimeError):
            list(vdlib.iter_chunks(self._file(vdlib.apply(old, 0, bad)), new, old, True))

    def test_vektor_kotlin_bilan_bir_xil(self):
        # Android sinovida (`CtrTest`) AYNAN shu vektor tekshiriladi.
        key = bytes.fromhex("00112233445566778899aabbccddeeff")
        plain = bytes(range(48))
        self.assertEqual(vdlib.apply(key, 32, plain).hex(), VEC)


VEC = "1df825344116b9d3974a61dc333f5c8acafe5de4f52850b1c3d20619526f4d9826c9479484130b08e9887c10eece2bab"

if __name__ == "__main__":
    unittest.main()
