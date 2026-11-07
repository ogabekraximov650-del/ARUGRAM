#!/usr/bin/env bash
# DoodStream API kalitini shifrlab `dood_key.enc` ga yozadi (tool/*/set_*.sh bilan bir xil usul: AES-256-CBC, PBKDF2 200 000, kalit — ENCODE_TOKEN).
#
#   ENCODE_TOKEN=<qiymati> bash ARUmultiplatform/tube/worker/set_key.sh
#
# Kalit so'raladi (ekranda ko'rinmaydi). Keyin `dood_key.enc` ni commit/push qiling — `tube-worker.yml` uni ochib Cloudflare secret qiladi.
set -euo pipefail
[ -n "${ENCODE_TOKEN:-}" ] || { echo "ENCODE_TOKEN berilmagan"; exit 1; }
read -rsp "DoodStream API kaliti: " KEY; echo
printf '%s' "$KEY" | K="$(printf '%s' "$ENCODE_TOKEN" | tr -d '[:space:]')" \
  openssl enc -aes-256-cbc -salt -pbkdf2 -iter 200000 -pass env:K -out "$(dirname "$0")/dood_key.enc"
echo "Tayyor: $(dirname "$0")/dood_key.enc"
