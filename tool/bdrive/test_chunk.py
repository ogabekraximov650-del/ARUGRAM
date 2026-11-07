"""chunk.py ning mahalliy sinovi (tarmoqsiz): multipart tuzilishi."""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import chunk  # noqa: E402


def test_multipart():
    body, ctype = chunk.multipart({"chat_id": "-100123", "disable_notification": "true"}, "document", "bd_a_0", b"\x00\x01abc")
    b = re.search(r"boundary=(\w+)", ctype).group(1).encode()
    assert body.startswith(b"--" + b + b"\r\n")
    assert b'name="chat_id"\r\n\r\n-100123\r\n' in body
    assert b'name="document"; filename="bd_a_0"' in body
    assert b"\r\n\r\n\x00\x01abc\r\n--" + b + b"--\r\n" in body


if __name__ == "__main__":
    test_multipart()
    print("ok")
