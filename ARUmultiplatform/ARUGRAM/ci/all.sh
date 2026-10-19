#!/usr/bin/env bash
# Bitta emulyator sessiyasida: skrinshotlar (screens.sh) va animatsiya videosi (anim.sh). Ishlatish: all.sh <apk> <chiqish_papkasi>
APK="$1"; OUT="$2"
D="$(dirname "$0")"
bash "$D/screens.sh" "$OUT/screens" "$APK"
bash "$D/anim.sh" "$OUT/anim"
