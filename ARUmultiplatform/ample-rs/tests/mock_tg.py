#!/usr/bin/env python3
"""UZ: sinov uchun soxta Telegram Bot API (yuklash, almashtirish, o'chirish, pin, yuklab olish). `GET /_state` — xabarlar soni. EN: a tiny fake Telegram Bot API for end-to-end backup/recovery tests."""
import json, sys, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOCK = threading.Lock()
MSGS = {}   # message_id -> {"file_id","name","data"}
FILES = {}  # file_id -> bytes
STATE = {"next": 1, "pinned": None, "reqs": 0, "up": 0}

def parse_multipart(ct, body):
    b = ct.split("boundary=")[1].strip().encode()
    parts = {}
    for chunk in body.split(b"--" + b):
        if not chunk.strip(b"\r\n-"): continue
        head, _, data = chunk.lstrip(b"\r\n").partition(b"\r\n\r\n")
        data = data[:-2] if data.endswith(b"\r\n") else data
        h = head.decode("latin1")
        name = h.split('name="')[1].split('"')[0]
        fname = h.split('filename="')[1].split('"')[0] if 'filename="' in h else None
        parts[name] = (fname, data)
    return parts

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def _send(self, obj, code=200):
        d = json.dumps(obj).encode(); self.send_response(code); self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(d))); self.end_headers(); self.wfile.write(d)
    def do_GET(self):
        if self.path == "/_state":
            with LOCK: return self._send({"messages": len(MSGS), "bytes": sum(len(m["data"]) for m in MSGS.values()), "pinned": STATE["pinned"], "reqs": STATE["reqs"], "up": STATE["up"], "names": sorted(m["name"] for m in MSGS.values())})
        if "/file/bot" in self.path:
            fid = self.path.split("/file/bot", 1)[1].split("/", 1)[1]
            with LOCK: d = FILES.get(fid)
            if d is None: return self._send({"ok": False}, 404)
            self.send_response(200); self.send_header("Content-Length", str(len(d))); self.end_headers(); self.wfile.write(d); return
        self._send({"ok": False}, 404)
    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0")); body = self.rfile.read(n)
        method = self.path.rsplit("/", 1)[1]
        ct = self.headers.get("Content-Type", "")
        with LOCK:
            STATE["reqs"] += 1
            if method == "sendDocument":
                p = parse_multipart(ct, body); fname, data = p["document"]
                mid = STATE["next"]; STATE["next"] += 1; fid = f"F{mid}"
                MSGS[mid] = {"file_id": fid, "name": fname, "data": data}; FILES[fid] = data; STATE["up"] += len(data)
                return self._send({"ok": True, "result": {"message_id": mid, "document": {"file_id": fid, "file_size": len(data)}}})
            if method == "editMessageMedia":
                p = parse_multipart(ct, body); mid = int(p["message_id"][1]); data = p["f"][1]
                m = MSGS.get(mid)
                if not m: return self._send({"ok": False, "error_code": 400, "description": "message not found"})
                m["file_id"] = f"F{mid}e{STATE['reqs']}"; m["data"] = data; FILES[m["file_id"]] = data; STATE["up"] += len(data)
                return self._send({"ok": True, "result": {"message_id": mid}})
            j = json.loads(body or b"{}")
            if method == "deleteMessage":
                MSGS.pop(j["message_id"], None); return self._send({"ok": True, "result": True})
            if method == "deleteMessages":
                for m in j["message_ids"]: MSGS.pop(m, None)
                return self._send({"ok": True, "result": True})
            if method == "pinChatMessage":
                STATE["pinned"] = j["message_id"]; return self._send({"ok": True, "result": True})
            if method == "getChat":
                m = MSGS.get(STATE["pinned"]) if STATE["pinned"] else None
                r = {"id": j["chat_id"]}
                if m: r["pinned_message"] = {"message_id": STATE["pinned"], "document": {"file_id": m["file_id"]}}
                return self._send({"ok": True, "result": r})
            if method == "getFile":
                return self._send({"ok": True, "result": {"file_path": j["file_id"]}})
        self._send({"ok": False, "error_code": 404, "description": "unknown " + method}, 404)

if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8099
    ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()
