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

# Animatsiya kadrlari tez chiqishi uchun yarim o'lcham (dp o'zgarmaydi): 360x775, 165 dpi. Koordinatlar yarmiga kamaygan.
adb shell wm size 360x775
adb shell wm density 165

shotf() { adb exec-out screencap -p > "$1"; }
# Sarlavha/matn qismi o'zgardimi (sahifa almashdimi)?
diffpng() { python3 - "$1" "$2" <<'PY'
import sys
from PIL import Image, ImageChops
a=Image.open(sys.argv[1]).convert("L").crop((0,310,360,420)); b=Image.open(sys.argv[2]).convert("L").crop((0,310,360,420))
d=sum(ImageChops.difference(a,b).histogram()[i]*i for i in range(256))/(360*110)
print("1" if d>3 else "0")
PY
}
METHOD=0
swipe_try() { # $1=x1 $2=x2 $3=method
  case "$3" in
    0) adb shell input swipe $1 350 $2 350 180;;
    1) adb shell input swipe $1 350 $2 350 500;;
    2) local x=$1 step=$(( ($2-$1)/8 ))
       adb shell input motionevent DOWN $1 350
       for k in 1 2 3 4 5 6 7 8; do x=$((x+step)); adb shell input motionevent MOVE $x 350; done
       adb shell input motionevent UP $2 350;;
  esac
}
page() { # $1 = left|right
  local x1=300 x2=60; [ "$1" = right ] && { x1=60; x2=300; }
  shotf "$OUT/_a.png"
  local m ch=0
  for m in $METHOD 0 1 2; do
    swipe_try $x1 $x2 $m; sleep 1.3
    shotf "$OUT/_b.png"
    if [ "$(diffpng "$OUT/_a.png" "$OUT/_b.png")" = 1 ]; then METHOD=$m; ch=1; break; fi
  done
  echo "surish usuli=$METHOD o'zgardi=$ch" >> "$OUT/methods.txt"
}

adb shell am force-stop "$PKG"; adb shell pm clear "$PKG" >/dev/null 2>&1 || true
for P in READ_PHONE_STATE READ_PHONE_NUMBERS CALL_PHONE READ_CALL_LOG RECEIVE_SMS READ_SMS POST_NOTIFICATIONS READ_CONTACTS; do adb shell pm grant "$PKG" "android.permission.$P" 2>/dev/null || true; done
adb shell am start -n "$ACT"; sleep 10
adb shell "screenrecord --time-limit 175 --bit-rate 12000000 /sdcard/anim.mp4" > "$OUT/screenrecord.log" 2>&1 &
REC=$!
T0=$(now)
ev "boshlandi"
sleep 6; ev "sahifa1_bo'sh_turish_tugadi"
for i in 2 3 4 5 6; do
  ev "surish_chapga_$i"; page left; sleep 6; ev "sahifa${i}_bo'sh_turish_tugadi"
done
for i in 5 4 3 2 1; do
  ev "surish_o'ngga_$i"; page right; sleep 2.5
done
ev "mavzu_1"; adb shell input tap 322 66; sleep 3
ev "mavzu_2"; adb shell input tap 322 66; sleep 3
ev "boshlash_bosildi"; adb shell input tap 180 656; sleep 5
ev "tugadi"
wait $REC 2>/dev/null || true
sleep 2
adb pull /sdcard/anim.mp4 "$OUT/anim.mp4" || true
adb logcat -d -t 3000 *:W | grep -E "AndroidRuntime|FATAL|$PKG|ANR|Process .* died" | tail -60 > "$OUT/logcat.txt" || true
adb shell dumpsys activity activities | grep -E "mResumedActivity|topResumedActivity" | head -3 >> "$OUT/logcat.txt" || true
