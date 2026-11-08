#!/usr/bin/env bash
# Run boshlanganidan `dub.py` ishni olgunicha botdagi holat xabarini yangilab turadi (`/api/dub/stage`).
# Workflow har bosqichda `$RUNNER_TEMP/stage.txt` ga matn yozadi; shu skript har ~15 soniyada uni o'tgan vaqt bilan yuboradi.
# `dub.py` ishga tushganda `$RUNNER_TEMP/stage.done` yaratiladi — skript to'xtaydi (keyin foiz xabarini `dub.py` o'zi yangilaydi).
T0=$(date +%s)
while [ ! -f "$RUNNER_TEMP/stage.done" ]; do
  S=$(cat "$RUNNER_TEMP/stage.txt" 2>/dev/null || echo "Run boshlandi")
  EL=$(( $(date +%s) - T0 ))
  MSG=$(printf '%s\n\n\xe2\x8f\xb1 run boshlanganidan: %d:%02d' "$S" $((EL / 60)) $((EL % 60)))
  jq -n --arg t "$MSG" '{text: $t}' | curl -fsS -m 20 -X POST -H "X-Encode-Token: $ENCODE_TOKEN" -H "User-Agent: arumediatv-dub" \
    -H "Content-Type: application/json" -d @- "$API_BASE/api/dub/stage" >/dev/null 2>&1 || true
  sleep 15
done
