#!/usr/bin/env bash
# anibla.uz login/parolini yangilash: `tool/anibla/creds.enc` ni qayta shifrlaydi.
#
#   ANIBLA_KEY=<GitHub'dagi ANIBLA_KEY qiymati> bash tool/anibla/set_creds.sh
#
# Login (telefon) va parol so'raladi, ekranda ko'rinmaydi. Keyin faylni
# commit/push qiling — worker deploy'i (`deploy-worker.yml`) yangisini o'rnatadi.
# Shifr: AES-256-CBC, kalit PBKDF2 (200 000 marta) — `gh_token.enc` bilan bir xil.
set -euo pipefail
[ -n "${ANIBLA_KEY:-}" ] || { echo "ANIBLA_KEY berilmagan"; exit 1; }
read -rp "Login (telefon, masalan 998901234567): " LOGIN
read -rsp "Parol: " PASS; echo
SITE="${ANIBLA_SITE:-https://anibla.uz}"
python3 -c 'import json,sys; print(json.dumps({"site": sys.argv[1], "login": sys.argv[2], "password": sys.argv[3]}), end="")' \
  "$SITE" "$LOGIN" "$PASS" \
  | K="$(printf '%s' "$ANIBLA_KEY" | tr -d '[:space:]')" openssl enc -aes-256-cbc -salt -pbkdf2 -iter 200000 -pass env:K \
      -out "$(dirname "$0")/creds.enc"
echo "Tayyor: $(dirname "$0")/creds.enc"
