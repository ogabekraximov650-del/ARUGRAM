"""Sinov: `python3 tool/anibla/test_stage.py` — qismlab shifrlab, ulab ochish asl faylni qaytaradi."""
import os
import secrets
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import stage  # noqa: E402


def main():
    assert stage.plan_parts(0) == []
    assert stage.plan_parts(10, 16) == [(0, 10)]
    assert stage.plan_parts(100, 32) == [(0, 32), (32, 32), (64, 32), (96, 4)]
    assert sum(n for _, n in stage.plan_parts(5_000_000_000)) == 5_000_000_000
    assert all(o % 16 == 0 for o, _ in stage.plan_parts(5_000_000_000))
    assert len(stage.plan_parts(stage.PART + 1)) == 2
    try:
        stage.plan_parts(100, 10)
        raise SystemExit("16 ga karrali bo'lmagan qism rad etilishi kerak edi")
    except ValueError:
        pass
    d = Path(tempfile.mkdtemp())
    data = os.urandom(10 * 1048576 + 12345)  # 16 ga karrali emas — oxirgi qism qisqa
    (d / "src").write_bytes(data)
    key = secrets.token_bytes(16)
    part = 4 * 1048576  # sinov uchun kichik qism (16 ga karrali)
    paths = []
    for i, (off, n) in enumerate(stage.plan_parts(len(data), part)):
        p = d / f"p{i}"
        assert stage.encrypt_range(d / "src", key, off, n, p) == n
        paths.append(p)
    assert len(paths) == 3
    sealed = b"".join(p.read_bytes() for p in paths)
    assert sealed != data  # shifrlangan
    assert stage.join_decrypt(paths, key, d / "out") == len(data)
    assert (d / "out").read_bytes() == data
    # `tool/encode/run.py::ctr_file` (kodlovchi) bilan mos: ulangan qismlarni IV=0 CTR bilan ochish asl faylni beradi.
    from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
    joined = d / "joined"
    joined.write_bytes(sealed)
    dec = Cipher(algorithms.AES(key), modes.CTR(b"\0" * 16)).encryptor()  # run.py ctr_file shu shifrlovchi
    assert dec.update(joined.read_bytes()) + dec.finalize() == data
    playable()
    print("test_stage: OK")


def playable():
    """Ochiladigan qismlar: kichik sinov video -> split_playable -> concat_parts; davomiylik saqlanadi, har qism o'zi ochiladi (ffprobe)."""
    import subprocess
    d = Path(tempfile.mkdtemp())
    src = d / "src.mp4"
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=25", "-f", "lavfi", "-i", "sine=frequency=440",
                    "-t", "40", "-g", "25", "-c:v", "libx264", "-b:v", "400k", "-c:a", "aac", str(src)], check=True)
    size = src.stat().st_size

    def dur(p):
        return float(subprocess.check_output(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(p)]).decode().strip())
    total = dur(src)
    parts = stage.split_playable(src, d / "segs", total, size, target=size // 4, limit=size)
    assert len(parts) >= 3, len(parts)
    assert all(dur(p) > 1 for p in parts)  # har qism o'zi ochiladigan video
    out = d / "joined.mp4"
    stage.concat_parts(parts, out)
    assert abs(dur(out) - total) < 2.0, (dur(out), total)
    print("playable: OK", len(parts), "qism")


if __name__ == "__main__":
    main()
