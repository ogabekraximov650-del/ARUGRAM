# Maxfiy ma'lumotlar (faqat shifrlangan holda)

Ochiq matnli hech narsa saqlanmaydi. Kalit zanjiri:

- `tg-key.enc` — RSA maxfiy kalit, `ENCODE_TOKEN` (GitHub Secret) bilan AES-256-CBC (PBKDF2, 200000) shifrlangan.
- `tg-key.pub.pem` — mos ochiq kalit.
- `tg2fa.rsa` — Telegram hisobining ikki bosqichli (cloud) paroli, ochiq kalit bilan RSA-OAEP(SHA-256) shifrlangan.
- `grammers_session.json.enc` — grammers sessiyasi (keyinroq), `ENCODE_TOKEN` bilan shifrlangan.

CI'da ochish:
```
K=$(printf '%s' "$ENCODE_TOKEN" | tr -d '[:space:]')
K="$K" openssl enc -d -aes-256-cbc -pbkdf2 -iter 200000 -pass env:K -in tg-key.enc -out priv.pem
openssl pkeyutl -decrypt -inkey priv.pem -pkeyopt rsa_padding_mode:oaep -pkeyopt rsa_oaep_md:sha256 -in tg2fa.rsa   # chiqishni log'ga yozmang (::add-mask::)
```
