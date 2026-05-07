#!/usr/bin/env bash
# verify_batch.sh — verify_batch.txt の各 (input, expected) を CLI に流し、fail だけ出力。
#
# 使い方: bash tools/verify_batch.sh [batch_file]
# default batch_file: tools/verify_batch.txt
# CLI binary は target/release/furigana.exe を想定 (alpha.6+)

set -euo pipefail
BATCH="${1:-tools/verify_batch.txt}"
CLI="${FURIGANA_CLI:-target/release/furigana.exe}"

if [ ! -x "$CLI" ]; then
  echo "ERROR: $CLI が無い (cargo build --release してください)" >&2
  exit 1
fi
if [ ! -f "$BATCH" ]; then
  echo "ERROR: $BATCH が無い" >&2
  exit 1
fi

pass=0
fail=0
total=0

while IFS=$'\t' read -r input expected note; do
  # 空行 / コメント行 / フォーマット崩れスキップ
  [ -z "${input:-}" ] && continue
  case "$input" in \#*) continue ;; esac
  [ -z "${expected:-}" ] && continue

  total=$((total + 1))
  actual=$("$CLI" lookup "$input" --mode hiragana 2>/dev/null || echo '<error>')

  if [ "$actual" = "$expected" ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    printf '\033[31m✗\033[0m %s\n' "$input"
    printf '   expected: %s\n' "$expected"
    printf '   actual:   %s\n' "$actual"
    [ -n "${note:-}" ] && printf '   note:     %s\n' "$note"
    echo
  fi
done < "$BATCH"

echo "─────────────────"
echo "total=$total pass=$pass fail=$fail"
[ "$fail" -eq 0 ]
