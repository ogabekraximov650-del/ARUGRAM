#!/bin/sh
# UZ: intro_gl ni yig'adi. EN: builds intro_gl. RU: собирает intro_gl.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
TG="${TG_DIR:-$HERE/../../../Telegram/TMessagesProj}"
gcc -O2 -I"$HERE/stub" -I"$TG/jni/intro" -o "$HERE/intro_gl" "$HERE/harness.c" "$TG/jni/intro/IntroRenderer.c" -lEGL -lGLESv2 -lm
