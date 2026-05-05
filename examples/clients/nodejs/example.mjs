#!/usr/bin/env node
/**
 * ja-furigana の HTTP API を Node.js から叩く最小サンプル。
 *
 * 事前準備:
 *   1. `furigana serve` をローカルで起動
 *   2. Node.js 18+ (組み込み fetch 使用、依存なし)
 *   3. このスクリプトを実行: `node example.mjs`
 *
 * 主用途: Discord bot で難読語のフリガナ補助、Web フロントエンドへのフォールバック等。
 */

const BASE_URL = "http://127.0.0.1:8000";
const API_KEY = null; // config.toml の [auth].tokens を設定した場合のみ

function headers() {
  const h = { "Content-Type": "application/json" };
  if (API_KEY) h["X-API-Key"] = API_KEY;
  return h;
}

/** `GET /healthz` で生死確認 + dict_size 取得。 */
async function healthz() {
  const r = await fetch(`${BASE_URL}/healthz`);
  if (!r.ok) throw new Error(`healthz failed: ${r.status}`);
  return r.json();
}

/**
 * `POST /furigana` で 1 ショット変換。
 *
 * @param {string} text - 変換対象の日本語テキスト
 * @param {"tts"|"hiragana"|"ruby"|"kanji"} [mode="tts"]
 * @returns {Promise<string>} 変換後テキスト
 */
async function lookup(text, mode = "tts") {
  const r = await fetch(`${BASE_URL}/furigana`, {
    method: "POST",
    headers: headers(),
    body: JSON.stringify({ text, mode }),
  });
  if (!r.ok) throw new Error(`lookup failed: ${r.status} ${await r.text()}`);
  const { result } = await r.json();
  return result;
}

async function main() {
  console.log("=== healthz ===");
  console.log(await healthz());

  console.log("\n=== ruby (1 件) ===");
  console.log(await lookup("灰桜の散る道", "ruby"));
  // → {灰桜|はいざくら}の{散る|ちる}{道|みち}

  console.log("\n=== Discord bot 風: 難読語の読みを返す ===");
  const messages = ["四面楚歌", "一期一会", "鹿児島", "1KMの道"];
  for (const m of messages) {
    const reading = await lookup(m, "hiragana");
    console.log(`  ${m}  →  ${reading}`);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
