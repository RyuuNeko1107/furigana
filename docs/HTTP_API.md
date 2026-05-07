# HTTP API

`furigana serve` はローカル HTTP サーバーです。default bind は `127.0.0.1:8000`。
パラメータ (`mode` / `text_b64` / `segmented` / `X-API-Key` 等) は下記のとおり。

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

未知の `mode` 値は **silently `tts` (default) にフォールバック** (エラーにはならない)。

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

1. `X-API-Key: <token>` (優先される)
2. `Authorization: Bearer <token>` (fallback)

`/healthz` は **常に認証不要**。

```sh
curl -H 'X-API-Key: secret' \
  'http://127.0.0.1:8000/furigana?text=灰桜&mode=ruby'
```

設定の詳細は [CONFIG.md](./CONFIG.md) を参照。

## ホットリロード / 自動更新

辞書を更新する経路は用途別に **5 通り**。一番簡単なのは「pull して再起動」(設定ゼロ)。

### A. プロセス再起動 (個人 / 開発、設定ゼロ、一番シンプル)

```sh
furigana dict pull            # GitHub Releases から最新を DL + flat 展開
# → 起動中の furigana を Ctrl-C で止めて再度起動 (起動時に新辞書をロード)
furigana serve
```

- **トークン不要 / config.toml 不要**
- ダウンタイム ~数秒 (起動コスト)
- 個人運用ならこれで十分

### B. `furigana serve --auto-pull` (起動時 1 回 pull、alpha.5+)

```sh
furigana serve --auto-pull
# → listen 開始前に dict pull を内部実行 → 失敗時は warn のみで起動継続
```

- **トークン不要 / config.toml 不要**
- network なし / GitHub 一時障害でも壊れない
- systemd の `Restart=on-failure` と組み合わせると、再起動するたびに最新を引いてくる

`config.toml [auto_update].pin` が空でなければそれを尊重 (空なら latest 追従)。

### C. `[auto_update]` 定期 polling (無停止運用、alpha.5+)

```toml
# config.toml
[auto_update]
enabled  = true
interval = "6h"      # 30m / 1h / 6h / 1d 等。1h 以上推奨 (GitHub API rate limit)
# pin = "v0.1.3"     # 空なら最新追従
```

- **トークン不要** (内部呼び出しで HTTP 経由しないため)
- background task が定期 polling → 新 tag があれば自動 pull + 自動 reload
- 既存リクエスト中も `RwLock<Arc<Furigana>>` swap で **ダウンタイムなし**
- failed tick は warn のみで稼働継続

### D. `SIGHUP` シグナル (Unix のみ)

```sh
furigana dict pull --version v0.1.3  # 先に DL
kill -HUP $(pgrep furigana)          # signal で reload
# systemd なら ExecReload=/bin/kill -HUP $MAINPID
```

- **トークン不要** (プロセス権限で signal 送信できる人だけ)
- Windows ではビルドから除外、E に移行

### E. `POST /admin/reload` (外部から HTTP で reload、admin_tokens 必須)

```sh
furigana dict pull --version v0.1.3

curl -X POST -H 'X-API-Key: <admin-token>' \
  http://127.0.0.1:8000/admin/reload
# → {"status":"reloaded","dict_size":44354}
```

- **`[auth].admin_tokens` 設定済みのときだけ動く**。未設定なら 503 返却で機能 off
- マルチプロセス / マルチホストで同期 reload を打ちたい運用者向け
- 認証: `X-API-Key` または `Authorization: Bearer`、admin_tokens に一致するもののみ通る
- 一般 `tokens` では通らない (`/admin/*` は完全に別系列)

### どれを選ぶ?

| 用途 | お勧め経路 |
|---|---|
| 個人 / 開発 / 趣味 | **A** (再起動)、または **B** (`--auto-pull`) |
| 個人サーバ・無停止運用 | **C** (`[auto_update]`) |
| Linux サーバ・systemd 連携 | **C** + **D** (auto_update + SIGHUP の併用) |
| 外部から reload を打つマルチ replica | **E** (`admin_tokens` 必須) |

## 他言語クライアント

`furigana serve` は普通の HTTP API なので、HTTP が話せる言語ならどこからでも使える。最小サンプルが [`examples/clients/`](../examples/clients/) にある:

- [Python (`requests`)](../examples/clients/python/example.py) — TTS パイプライン / NLP 系
- [Node.js (組込 `fetch`)](../examples/clients/nodejs/example.mjs) — Discord bot / Web フロント
- [curl + bash](../examples/clients/curl/example.sh) — shell パイプ / 動作確認用

C++ / C# / Go / Ruby などはこれらをテンプレに好きな HTTP クライアントで。
