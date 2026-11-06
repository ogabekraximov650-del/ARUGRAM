"""Telegram Bot API (bulutli api.telegram.org) uchun yengil mijoz. Fayl yuborish — multipart (har bo'lak <= 50 MB)."""
import asyncio
import json
import logging

import aiohttp

log = logging.getLogger("anibot.tg")
API = "https://api.telegram.org"


class TgError(Exception):
    def __init__(self, method, desc, code=0, retry_after=0):
        super().__init__(f"{method}: {desc}")
        self.desc, self.code, self.retry_after = desc, code, retry_after


class Tg:
    def __init__(self, token):
        self.token = token
        self._s = None

    async def session(self):
        if self._s is None or self._s.closed:
            # Yuklash uzoq davom etishi mumkin (50 MB, sekin tarmoq) — umumiy timeout yo'q, faqat ulanish.
            self._s = aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=None, connect=30, sock_read=300))
        return self._s

    async def close(self):
        if self._s and not self._s.closed:
            await self._s.close()

    async def _do(self, method, make_body, tries=4):
        url = f"{API}/bot{self.token}/{method}"
        last = None
        for attempt in range(tries):
            try:
                s = await self.session()
                async with s.post(url, data=make_body()) as r:
                    j = await r.json(content_type=None)
                if j.get("ok"):
                    return j["result"]
                p = j.get("parameters") or {}
                err = TgError(method, str(j.get("description", "")), int(j.get("error_code", 0)), int(p.get("retry_after", 0)))
                if err.code == 429 and attempt < tries - 1:
                    await asyncio.sleep(min(err.retry_after + 1, 120))
                    last = err
                    continue
                raise err
            except TgError:
                raise
            except (aiohttp.ClientError, asyncio.TimeoutError, json.JSONDecodeError) as e:
                last = e
                log.warning("%s tarmoq xatosi (%s/%s): %s", method, attempt + 1, tries, e)
                await asyncio.sleep(2 * (attempt + 1))
        raise TgError(method, f"tarmoq: {last}")

    async def call(self, method, **params):
        body = json.dumps({k: v for k, v in params.items() if v is not None})

        def make():
            return aiohttp.payload.StringPayload(body, content_type="application/json")
        return await self._do(method, make)

    async def upload(self, method, fields, files):
        """files: {maydon: (yo'l, fayl_nomi)} — har urinishda fayl qayta ochiladi."""
        def make():
            fd = aiohttp.FormData()
            for k, v in fields.items():
                if v is not None:
                    fd.add_field(k, str(v))
            for k, (path, name) in files.items():
                fd.add_field(k, open(path, "rb"), filename=name)
            return fd
        return await self._do(method, make, tries=3)

    # ── qulay o'ramlar ───────────────────────────────────────
    async def send_message(self, chat, text, kb=None):
        return await self.call("sendMessage", chat_id=chat, text=text, parse_mode="HTML",
                               disable_web_page_preview=True, reply_markup=kb)

    async def edit_message(self, chat, mid, text, kb=None):
        try:
            return await self.call("editMessageText", chat_id=chat, message_id=mid, text=text, parse_mode="HTML",
                                   disable_web_page_preview=True, reply_markup=kb)
        except TgError as e:
            if "not modified" in e.desc:
                return None
            raise

    async def delete_message(self, chat, mid):
        try:
            await self.call("deleteMessage", chat_id=chat, message_id=mid)
        except TgError:
            pass

    async def answer_cb(self, cb_id, text=""):
        try:
            await self.call("answerCallbackQuery", callback_query_id=cb_id, text=text or None)
        except TgError:
            pass

    async def send_video(self, chat, path, caption, duration=0, width=0, height=0, thumb=None):
        files = {"video": (path, "video.mp4")}
        if thumb:
            files["thumbnail"] = (thumb, "thumb.jpg")
        return await self.upload("sendVideo", {
            "chat_id": chat, "caption": caption[:1000], "parse_mode": "HTML", "supports_streaming": "true",
            "duration": int(duration) or None, "width": width or None, "height": height or None,
        }, files)
