# HTTP API

`furigana serve` は本番 [ryuuneko.com のフリガナ API](https://ryuuneko.com/?slug=furigana-api) と互換のローカル HTTP サーバーです。default bind は `127.0.0.1:8000`。

> 戻る: [README](../README.md) / 関連: [CONFIG.md](./CONFIG.md) (auth / cors)

## 起動

```sh
furigana serve                                 # 127.0.0.1:8000
furigana serve --bind 0.0.0.0:8000             # 外部からも叩く
FURIGANA_TOKEN=<secret> furigana serve         # 認証有効
```

## エンドポイント

### `GET /healthz`

認証不要。dict_size でデータが mount されているか確認できる。

```json
{"status": "ok", "dict_size": 44354}
```

`dict_size` が 0 なら **辞書未配置**: 形態素解析は動くが熟語 hit / 助数詞 / 文脈ルールは無効の degraded mode。`furigana dict pull` で取得を促す。

### `GET /furigana?text=...&mode=...`

```sh
curl 'http://127.0.0.1:8000/furigana?text=灰桜の道&mode=ruby'
```

```json
{
  "result": "{灰桜|はいざくら}の{道|みち}",
  "mode": "ruby"
}
```

### `POST /furigana`

```sh
curl -X POST http://127.0.0.1:8000/furigana \
  -H 'Content-Type: application/json' \
  -d '{"text":"灰桜の道","mode":"ruby"}'
```

### `POST /admin/reload`

辞書を再 build して in-memory state を swap (hot reload)。`[auth].admin_tokens` 認証が必須。詳細は [hot reload](#ホットリロード) を参照。

```json
{"status": "reloaded", "dict_size": 44354}
```

## パラメータ

| パラメータ | 型 | default | 説明 |
|---|---|---|---|
| `text` | string | — | 変換対象。`text` または `text_b64` のどちらか必須 |
| `text_b64` | string | — | URL-safe base64 (`+` / `=` 含む文字列を URL に乗せる用) |
| `mode` | string | `tts` | 後述の 6 種 |
| `short_pause` | string | `" "` | TTS: 「、」後に挿入する文字列 |
| `long_pause` | string | `"   "` | TTS: 「。!?」後に挿入する文字列 |
| `keep_period` | bool | `true` | TTS: 末尾の `。` を残すか |
| `segmented` | bool | `false` | `tts` / `hiragana` のとき分割配列を `segments` に同梱 |
| `max_segment_len` | int | `60` | `segmented=true` のときの 1 セグメント最大文字数 |
| `debug` | bool | `false` | `timings_ms` を同梱 (tokenize / convert / total) |

### `mode` 一覧

| 値 | 出力 |
|---|---|
| `tts` (default) | TTS 整形ひらがな (ポーズ込み) |
| `hiragana` | プレーンひらがな (ポーズなし) |
| `ruby` | `{漢字|ひらがな}` 形式 |
| `kanji` | 入力をそのまま (no-op) |
| `romaji` | ヘボン式ローマ字 |
| `romaji-kunrei` | 訓令式ローマ字 |

未知の `mode` 値は **silently `tts` (default) にフォールバック** (本番 ryuuneko.com API と同挙動、エラーにはならない)。

## エラー応答

```jsonc
// 400 Bad Request — text が空
{"error":"no text provided"}

// 400 Bad Request — text が長すぎる (> 10,000 文字)
{"error":"text too long: 12345 chars (max 10000)"}

// 400 Bad Request — text_b64 のデコード失敗
{"error":"invalid base64 in text_b64"}

// 400 Bad Request — text_b64 が UTF-8 として不正
{"error":"text_b64 decoded bytes are not valid UTF-8"}

// 401 Unauthorized — `[auth].tokens` 設定済みで X-API-Key / Bearer 不一致
//                    (status のみ、本文なし)

// 503 Service Unavailable — `/admin/reload` で `[auth].admin_tokens` 未設定
//                           (status のみ、本文なし、admin 機能 off の合図)
```

最大入力長は 10,000 文字。

## 認証

`config.toml` の `[auth].tokens` または起動時 `--token` (env `FURIGANA_TOKEN`) に 1 つ以上のトークンを設定すると、`/furigana` で認証必須化。

**ヘッダ優先順位** (どちらか 1 つあれば OK):

1. `X-API-Key: <token>` (公開 API 互換、優先される)
2. `Authorization: Bearer <token>` (fallback)

`/healthz` は **常に認証不要**。

```sh
curl -H 'X-API-Key: secret' \
  'http://127.0.0.1:8000/furigana?text=灰桜&mode=ruby'
```

設定の詳細は [CONFIG.md](./CONFIG.md) を参照。

## ホットリロード

辞書を再読込する 2 経路:

### `POST /admin/reload`

`[auth].admin_tokens` (一般 `tokens` とは別系列) で認証。

```sh
curl -X POST -H 'X-API-Key: <admin-token>' \
  http://127.0.0.1:8000/admin/reload
# → {"status":"reloaded","dict_size":44354}
```

`admin_tokens` が未設定の場合は **503 Service Unavailable** を返して機能 off (一般利用者がうっかり叩いても害がない設計)。

### `SIGHUP` (Unix のみ)

```sh
kill -HUP $(pgrep furigana)
# systemd なら ExecReload=/bin/kill -HUP $MAINPID
```

Windows ではビルドから除外。`POST /admin/reload` を使う。

### 想定フロー

```sh
# 1. 新しい辞書版を取得
furigana dict pull --version v0.1.2

# 2. サーバープロセスに reload を促す
curl -X POST -H 'X-API-Key: <admin-token>' http://127.0.0.1:8000/admin/reload
# あるいは Unix なら kill -HUP <pid>
```

ダウンタイムなしで辞書差し替え可能。

## 他言語クライアント

`furigana serve` は普通の HTTP API なので、HTTP が話せる言語ならどこからでも使える。最小サンプルが [`examples/clients/`](../examples/clients/) にある:

- [Python (`requests`)](../examples/clients/python/example.py) — TTS パイプライン / NLP 系
- [Node.js (組込 `fetch`)](../examples/clients/nodejs/example.mjs) — Discord bot / Web フロント
- [curl + bash](../examples/clients/curl/example.sh) — shell パイプ / 動作確認用

C++ / C# / Go / Ruby などはこれらをテンプレに好きな HTTP クライアントで。
