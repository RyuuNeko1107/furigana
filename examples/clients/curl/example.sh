#!/usr/bin/env bash
# ja-furigana の HTTP API を curl から叩く最小サンプル。
#
# 事前準備:
#   1. `furigana serve` をローカルで起動
#   2. (必要なら) `jq` を入れておくと結果整形が楽
#
# 使い方:
#   bash example.sh

set -euo pipefail

BASE_URL="http://127.0.0.1:8000"
# API_KEY="your-token"   # config.toml の [auth].tokens を設定した場合のみ

# 認証ヘッダ (空なら付けない)
auth_header=()
if [[ -n "${API_KEY-}" ]]; then
  auth_header=(-H "X-API-Key: ${API_KEY}")
fi

echo "=== healthz ==="
curl -s "${BASE_URL}/healthz" "${auth_header[@]}"
echo

echo
echo "=== GET 形式 (URL に text を載せる) ==="
text=$(printf '灰桜の散る道' | jq -sRr @uri)
curl -s "${auth_header[@]}" "${BASE_URL}/furigana?text=${text}&mode=ruby"
echo

echo
echo "=== POST 形式 (JSON body) ==="
curl -s -X POST "${BASE_URL}/furigana" \
  -H 'Content-Type: application/json' \
  "${auth_header[@]}" \
  -d '{"text":"今日は5KMを30分で走った","mode":"tts"}'
echo

echo
echo "=== shell パイプ的な使い方 (.result だけ取り出す) ==="
result=$(curl -s -X POST "${BASE_URL}/furigana" \
  -H 'Content-Type: application/json' \
  "${auth_header[@]}" \
  -d '{"text":"四面楚歌で一期一会","mode":"hiragana"}' \
  | jq -r '.result')
echo "→ ${result}"
