#!/usr/bin/env bash
# Kirish oynasi animatsiyalarini emulyatorda VIDEOGA oladi (DrKLO Telegram va ARUGRAM uchun bir xil ssenariy). Har bir harakat vaqti events.txt ga
# yoziladi (videoning boshiga nisbatan sekund) — kadrlarni solishtirishda shu vaqtlardan foydalaniladi.
# Muhit: PKG, ACT (ishga tushirish komponenti). Ishlatish: anim.sh <chiqish_papkasi>
set -u
OUT="${1:-anim}"; mkdir -p "$OUT"
PKG="${PKG:-uz.arugram.app}"; ACT="${ACT:-uz.arugram.app/.MainActivity}"; case "$ACT" in */*) ;; *) ACT="$PKG/$ACT";; esac
EV="$OUT/events.txt"; : > "$EV"
now() { date +%s.%N; }
T0=0
ev() { printf '%.2f %s\n' "$(echo "$(now) - $T0" | bc -l)" "$1" >> "$EV"; }
sig() { adb exec-out screencap -p | md5sum | cut -c1-12; }

# Bir sahifa chapga/o'ngga: avval tez surish, o'zgarmasa sekinroq, so'ng sudrab tashlash.
page() { # $1 = left|right
  local x1=600 x2=120; [ "$1" = right ] && { x1=120; x2=600; }
  adb shell input swipe $x1 700 $x2 700 180; sleep 1.2
}

adb shell am force-stop "$PKG"; adb shell pm clear "$PKG" >/dev/null 2>&1 || true
for P in READ_PHONE_STATE READ_PHONE_NUMBERS CALL_PHONE READ_CALL_LOG RECEIVE_SMS READ_SMS POST_NOTIFICATIONS READ_CONTACTS; do adb shell pm grant "$PKG" "android.permission.$P" 2>/dev/null || true; done
adb shell am start -n "$ACT"; sleep 10
adb shell "screenrecord --time-limit 175 --bit-rate 12000000 /sdcard/anim.mp4" &
REC=$!
T0=$(now)
ev "boshlandi"
sleep 6; ev "sahifa1_bo'sh_turish_tugadi"
for i in 2 3 4 5 6; do
  ev "surish_chapga_$i"; page left; sleep 8; ev "sahifa${i}_bo'sh_turish_tugadi"
done
for i in 5 4 3 2 1; do
  ev "surish_o'ngga_$i"; page right; sleep 2.5
done
ev "mavzu_1"; adb shell input tap 645 132; sleep 3
ev "mavzu_2"; adb shell input tap 645 132; sleep 3
ev "boshlash_bosildi"; adb shell input tap 360 1312; sleep 5
ev "tugadi"
wait $REC 2>/dev/null || true
sleep 2
adb pull /sdcard/anim.mp4 "$OUT/anim.mp4" || true
