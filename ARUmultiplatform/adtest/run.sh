#!/usr/bin/env bash
# UZ: emulyatorda ADTEST ilovasini o'rnatadi, ishga tushiradi va logcat'dan natijani o'qiydi. EN/RU: installs and runs ADTEST, reads the logcat result.
set -u
APK="ARUmultiplatform/adtest/app/build/outputs/apk/debug/app-debug.apk"
adb wait-for-device
adb install -r "$APK"
adb logcat -c
adb shell am start -n uz.arumediatv.adtest/.MainActivity
for i in $(seq 1 60); do
  sleep 3
  if adb logcat -d -s ADTEST:I | grep -q ADTEST_DONE; then break; fi
done
echo "================ ADTEST NATIJA ================"
adb logcat -d -s ADTEST:I | grep -E "ADTEST_" || echo "ADTEST yozuvi yo'q (ilova ochilmadi?)"
echo "================================================"
adb logcat -d -s AndroidRuntime:E | head -30
if adb logcat -d -s ADTEST:I | grep -q "ADTEST_RESULT control LOADED"; then echo "SDK SOG'LOM: sinov reklamasi yuklandi."; else echo "DIQQAT: sinov reklamasi ham yuklanmadi (SDK/tarmoq muammosi)."; exit 1; fi
