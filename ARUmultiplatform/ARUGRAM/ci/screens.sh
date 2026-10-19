#!/usr/bin/env bash
# Emulyatorda ARUGRAM'ni ochib, kirish oynasining skrinshotlari va videosini oladi (Telegram bilan solishtirish uchun).
# Ekran Telegram skrinshotlari bilan bir xil: 329 dpi (dp = 2.056 px). Telegram konteyneri 1500 px balandlikda (pastda 100 px qora navigatsiya); bu yerda gesture-navigatsiya + 1549 px ekran.
set -u
OUT="${1:-screens}"; APK="${2:-app.apk}"
PKG="${PKG:-uz.arugram.app}"; ACT="${ACT:-uz.arugram.app/.MainActivity}"; case "$ACT" in */*) ;; *) ACT="$PKG/$ACT";; esac
mkdir -p "$OUT"
adb wait-for-device
adb shell wm size "${SCREEN_SIZE:-720x1549}"
adb shell wm density 329   # Telegram skrinshotidagi qurilma: dp = 2.056 px (tugma 320 dp = 658 px)
adb shell cmd overlay enable com.android.internal.systemui.navbar.gestural || true
sleep 3
adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null 2>&1 || true
adb shell cmd uimode night no || true
adb shell settings put global window_animation_scale 1
adb shell settings put global transition_animation_scale 1
adb shell settings put global animator_duration_scale 1
adb install -r "$APK"
for P in READ_PHONE_STATE READ_PHONE_NUMBERS CALL_PHONE READ_CALL_LOG RECEIVE_SMS READ_SMS POST_NOTIFICATIONS READ_CONTACTS; do adb shell pm grant "$PKG" "android.permission.$P" 2>/dev/null || true; done
shot() { adb exec-out screencap -p > "$OUT/$1.png"; }
tap() { adb shell input tap "$1" "$2"; }
swipe_left() { adb shell input swipe 600 700 120 700 150; }

adb shell input keyevent KEYCODE_HOME
sleep 3
adb shell am force-stop "$PKG"
adb shell am start -n "$ACT"
sleep 6
adb shell "screenrecord --time-limit 80 --bit-rate 8000000 /sdcard/intro.mp4" &
REC=$!
sleep 2
shot 01_light_page1
for i in 2 3 4 5 6; do
  swipe_left; sleep 1.5; shot "01_light_page$i"
done
# Mavzuni almashtirish: tugma (645,132); ochilish animatsiyasi o'rtasida va oxirida kadr
tap 645 132; sleep 0.25; shot 02_theme_mid; sleep 3; shot 02_dark_page6
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
adb shell dumpsys window | grep -iE "navigationBars|statusBars|mFrame=|Display" | head -40 > "$OUT/insets.txt" || true
adb logcat -d -s AndroidRuntime:E ArugramApp:V > "$OUT/logcat.txt" || true
