# 設定ファイル / 環境変数 / CLI フラグ

`furigana` CLI の設定ソース 3 経路と、それぞれの優先順位の解説。

> 戻る: [README](../README.md) / 関連: [HTTP_API.md](./HTTP_API.md) / [DATA_LAYOUT.md](./DATA_LAYOUT.md)

## 設定ファイル (`config.toml`)

`<data_dir>/config.toml` を読みます。default の `<data_dir>` は実行ファイルと同じディレクトリ ([DATA_LAYOUT.md](./DATA_LAYOUT.md) 参照)。`--config <path>` または `FURIGANA_CONFIG` 環境変数で別の場所を指定可能。

ファイルが存在しなければ default 値で起動 (全項目 optional):

```toml
[server]
bind = "127.0.0.1:8000"
cors_origins = []  # 空 = Any 許可 (ローカル用途) / 厳格化したいときは ["https://example.com"]

[auth]
tokens = []        # 空 = /furigana 認証無効 (ローカル想定)
admin_tokens = []  # 空 = /admin/* 機能 off (503)
```

### `[server]` セクション

| キー | 型 | default | 説明 |
|---|---|---|---|
| `bind` | string | `"127.0.0.1:8000"` | `furigana serve` の listen address |
| `cors_origins` | string[] | `[]` | CORS 許可オリジン。空なら **Any 許可** (ローカル用途で楽)。本番運用なら明示指定推奨 |

### `[auth]` セクション

| キー | 型 | default | 説明 |
|---|---|---|---|
| `tokens` | string[] | `[]` | `/furigana` 用トークン。空なら認証無効 |
| `admin_tokens` | string[] | `[]` | `/admin/reload` 用トークン。空なら admin 機能自体が 503 で off |

`tokens` と `admin_tokens` は **別系列**: 一般 `tokens` では `/admin/*` は通らない。これにより「読み取りはできるが reload はさせない」運用が可能。

詳細な認証フローは [HTTP_API.md#認証](./HTTP_API.md#認証) / [HTTP_API.md#ホットリロード](./HTTP_API.md#ホットリロード) を参照。

## 環境変数

`config.toml` より後で評価され、上書きする:

| 変数 | 役割 | 例 |
|---|---|---|
| `FURIGANA_DATA_DIR` | `<data_dir>` を上書き | `/var/lib/furigana` |
| `FURIGANA_CONFIG` | `config.toml` の path を上書き | `/etc/furigana/config.toml` |
| `FURIGANA_TOKEN` | `[auth].tokens` に 1 件追加 | `secret-token-xyz` |
| `RUST_LOG` | tracing log level | `info`, `debug`, `furigana=trace` |

`FURIGANA_TOKEN` は **既存 `tokens` を置き換えるのではなく追加** する。複数 token 運用にも干渉しない。

## CLI フラグ (最優先)

```sh
furigana --data-dir /var/lib/furigana \
         --config /etc/furigana/config.toml \
         --verbose \
         serve --bind 0.0.0.0:8000 --token secret
```

| フラグ | 役割 | 環境変数 fallback |
|---|---|---|
| `--data-dir <path>` | `<data_dir>` を上書き (グローバル) | `FURIGANA_DATA_DIR` |
| `--config <path>` | `config.toml` の path を上書き (グローバル) | `FURIGANA_CONFIG` |
| `-v, --verbose` | tracing log を info にする (グローバル) | (なし) |
| `serve --bind <addr>` | listen address を上書き | (なし) |
| `serve --token <t>` | 一般 token を 1 件追加 | `FURIGANA_TOKEN` |

CLI flag → env → `config.toml` → default の優先順位で評価される。

## 設定例

### ローカル開発 (default のまま)

`config.toml` 不要、環境変数も不要。`furigana serve` で `127.0.0.1:8000` に listen、認証なし。

### 信頼できる LAN 内サーバー (token のみ)

```toml
# config.toml
[server]
bind = "0.0.0.0:8000"

[auth]
tokens = ["lan-shared-token-xxxxxx"]
```

```sh
furigana --data-dir /var/lib/furigana serve
```

### 公開 API + reload 経路を分離

```toml
# config.toml
[server]
bind = "0.0.0.0:8000"
cors_origins = ["https://your-frontend.example.com"]

[auth]
tokens = ["public-readonly-token"]
admin_tokens = ["ops-only-reload-token"]  # 別系列、reload 専用
```

```sh
# 通常の lookup
curl -H 'X-API-Key: public-readonly-token' \
  'https://api.example.com/furigana?text=灰桜&mode=ruby'

# 辞書 reload (admin 専用)
curl -X POST -H 'X-API-Key: ops-only-reload-token' \
  https://api.example.com/admin/reload
```

### Docker

```sh
docker run -d -p 8000:8000 \
  -v /host/data:/data \
  -e FURIGANA_DATA_DIR=/data \
  -e FURIGANA_TOKEN=secret \
  ghcr.io/ryuuneko1107/furigana:latest \
  furigana serve --bind 0.0.0.0:8000
```

`/host/data` を mount して、`furigana dict pull` の結果を host に永続化する想定。
