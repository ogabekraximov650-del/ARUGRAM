#!/usr/bin/env bash
# E'lon kanali (yangi qism chiqqanda post tushadigan kanal): `tool/announce/channel.enc` ni qayta shifrlaydi.
#
#   ANNOUNCE_KEY=<ENCODE_TOKEN qiymati> bash tool/announce/set_channel.sh -1001234567890
#
# Keyin faylni commit/push qiling — worker deploy'i (`deploy-worker.yml`) `ANNOUNCE_CHANNEL` ni o'rnatadi.
# Shifr: AES-256-CBC, kalit PBKDF2 (200 000 marta) — `creds.enc`, `gh_token.enc` bilan bir xil.
set -euo pipefail
[ -n "${ANNOUNCE_KEY:-}" ] || { echo "ANNOUNCE_KEY berilmagan"; exit 1; }
CH="${1:-}"
[[ "$CH" =~ ^-?[0-9]+$ ]] || { echo "Kanal ID raqam bo'lishi kerak (masalan -1001234567890)"; exit 1; }
printf '{"channel_id": %s}' "$CH" \
  | K="$(printf '%s' "$ANNOUNCE_KEY" | tr -d '[:space:]')" openssl enc -aes-256-cbc -salt -pbkdf2 -iter 200000 -pass env:K \
      -out "$(dirname "$0")/channel.enc"
echo "Tayyor: $(dirname "$0")/channel.enc"
