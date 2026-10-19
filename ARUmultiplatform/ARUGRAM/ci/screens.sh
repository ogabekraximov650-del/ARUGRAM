#!/usr/bin/env bash
# Emulyatorda ARUGRAM'ni ochib, kirish oynasining skrinshotlari va videosini oladi (Telegram bilan solishtirish uchun).
# Ekran Telegram skrinshotlari bilan bir xil: 720x1600, 320 dpi (360 dp), 3 tugmali navigatsiya.
set -u
OUT="${1:-screens}"; APK="${2:-app.apk}"
mkdir -p "$OUT"
adb wait-for-device
adb shell wm size 720x1600
adb shell wm density 320
adb shell cmd overlay enable com.android.internal.systemui.navbar.threebutton || true
adb shell cmd uimode night no || true
adb shell settings put global window_animation_scale 1
adb shell settings put global transition_animation_scale 1
adb shell settings put global animator_duration_scale 1
adb install -r "$APK"
adb shell pm grant uz.arugram.app android.permission.READ_PHONE_STATE 2>/dev/null || true
shot() { adb exec-out screencap -p > "$OUT/$1.png"; }
tap() { adb shell input tap "$1" "$2"; }
swipe_left() { adb shell input swipe 600 700 120 700 150; }

adb shell am force-stop uz.arugram.app
adb shell am start -n uz.arugram.app/.MainActivity
sleep 6
adb shell "screenrecord --time-limit 80 --bit-rate 8000000 /sdcard/intro.mp4" &
REC=$!
sleep 2
shot 01_light_page1
for i in 2 3 4 5 6; do
  swipe_left; sleep 1.5; shot "01_light_page$i"
done
# Mavzuni almashtirish: tugma (645,132); ochilish animatsiyasi o'rtasida va oxirida kadr
tap 645 132; sleep 0.25; shot 02_theme_mid; sleep 1.5; shot 02_dark_page6
for i in 1 2 3 4 5; do
  adb shell input swipe 120 700 600 700 150; sleep 1.5; shot "03_dark_page$((6-i))"
done
# Til tugmasi
tap 360 1171; sleep 1; shot 04_english
tap 360 1171; sleep 1
# "Suhbatni boshlash": tugma dumaloq tugmaga aylanadi
tap 360 1312; sleep 0.12; shot 05_morph_a; sleep 0.12; shot 05_morph_b; sleep 1.5; shot 06_login_phone
wait $REC 2>/dev/null || true
sleep 2
adb pull /sdcard/intro.mp4 "$OUT/intro.mp4" || true
adb logcat -d -s AndroidRuntime:E ArugramApp:V > "$OUT/logcat.txt" || true
