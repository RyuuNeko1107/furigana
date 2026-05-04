# furigana

[![CI](https://github.com/RyuuNeko1107/furigana/actions/workflows/ci.yml/badge.svg)](https://github.com/RyuuNeko1107/furigana/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)

> Japanese furigana (ruby) lookup library and HTTP server in Rust — fully data-driven rules, no DB required.

日本語テキストに **フリガナ (読み仮名 / ルビ)** を付けるための Rust 製ライブラリ + ローカル HTTP サーバー。

> ⚠️ **Status**: Pre-alpha — 開発中。API・データ形式は変更されます。

---

## 目次

- [なにこれ](#なにこれ)
- [なぜ作るのか](#なぜ作るのか)
- [クイックスタート](#クイックスタート)
- [辞書の置き場所](#辞書の置き場所)
- [ルール一覧 (全部データ)](#ルール一覧-全部データ)
- [HTTP API](#http-api)
- [設定ファイル](#設定ファイル)
- [アーキテクチャ概要](#アーキテクチャ概要)
- [ステータスとロードマップ](#ステータスとロードマップ)
- [ライセンス / コントリビュート](#ライセンス)

---

## なにこれ

- **ライブラリ** (`furigana` crate): `cargo add furigana` で組み込める。DB 不要、async 不要、Pure Rust。
- **CLI / ローカル サーバー** (`furigana-cli` → `furigana` バイナリ): ローカル HTTP API + 辞書管理コマンド。

**ローカル利用 / 組み込み用途** を前提に設計しています。デフォルト bind は `127.0.0.1`、認証なし、レート制限なし。公開サービスではなく、自分のアプリ・社内ツール・スクリプトに組み込む使い方を想定。

## なぜ作るのか

既存のフリガナツールはだいたい次のどちらか:

- (a) ルール (助数詞・連濁・文脈読み等) を全部 Rust ソースに直書きしているため、辞書追加・修正に **再コンパイル必須**、Rust 知識ない人は PR を投げられない
- (b) Postgres / Java など重量級依存があり、「ちょっと試す」ハードルが高い

`furigana` は、助数詞ルール・スケール・SI 単位・文脈依存読み・例外語句といった **すべてのルールを編集可能な TOML / TSV データファイル** として外出ししています。読みを追加したい? TSV を編集。助数詞ルールを直したい? TOML を編集。再コンパイル不要。

## クイックスタート

### ライブラリとして使う

`Cargo.toml`:

```toml
[dependencies]
furigana = "0.1"
```

```rust
use furigana::Furigana;

let mut f = Furigana::minimal()?;
f.add_reading("灰桜", "ハイザクラ");

println!("{}", f.to_ruby("灰桜の散る道"));
// → "{灰桜|はいざくら}の{散る|ちる}{道|みち}"

println!("{}", f.to_hiragana("灰桜の散る道"));
// → "はいざくらのちるみち"
```

builder API で辞書ディレクトリを指定:

```rust
use furigana::Furigana;

let f = Furigana::builder()
    .core_dict_dir("/path/to/dict/core")
    .user_dict_dir("/path/to/dict/user")
    .overrides_file("/path/to/overrides.tsv")
    .build()?;
```

サンプル: [`crates/furigana/examples/basic.rs`](crates/furigana/examples/basic.rs)

```sh
$ cargo run -p furigana --example basic
```

### CLI として使う

```sh
$ cargo install furigana-cli           # crates.io 公開時
# または GitHub Releases からバイナリ DL (準備中)

# 1 ショット変換
$ furigana lookup '灰桜の散る道'
{灰|はい}{桜|さくら}の{散る|ちる}{道|みち}

$ furigana lookup '灰桜の散る道' --format hiragana
はいさくらのちるみち

# 辞書追加
$ furigana dict add 灰桜 ハイザクラ
追加: 灰桜 → ハイザクラ
保存先: ~/.local/share/furigana/dict/user/cli-added.tsv

# 辞書反映後に再変換
$ furigana lookup '灰桜の散る道'
{灰桜|はいざくら}の{散る|ちる}{道|みち}

# サーバー起動
$ furigana serve
INFO furigana serving on http://127.0.0.1:8000
INFO Bearer 認証: 無効 (ローカル想定)
```

## 辞書の置き場所

```
~/.local/share/furigana/dict/      (Windows: %LOCALAPPDATA%\furigana\dict\)
├── core/                          # `furigana dict pull` で配布版を取得 (準備中)
│   ├── ja_auto.tsv
│   └── unihan_kana.tsv
├── user/                          # ユーザーが自由に *.tsv を置く
│   └── cli-added.tsv              # `furigana dict add` の保存先
└── overrides.tsv                  # 強制上書き用 (最優先)
```

優先順位 (高→低):

1. `overrides.tsv` (FuriganaBuilder の `overrides_file()`)
2. `user/*.tsv` (FuriganaBuilder の `user_dict_dir()`)
3. `core/*.tsv` (FuriganaBuilder の `core_dict_dir()`)
4. 文脈ルール (`data/rules/context.toml`)
5. Lindera (形態素解析) の読み
6. 何もなければ読みなし (`None`) — 出力では surface のまま

## ルール一覧 (全部データ)

| ファイル | 内容 |
|---|---|
| [`data/rules/counters.toml`](data/rules/counters.toml) | 助数詞 (本/匹/個/年/月/日…) の連濁・促音化・kana 末尾置換 |
| [`data/rules/days.toml`](data/rules/days.toml) | 1〜31 日の特殊読み (1→ツイタチ 等) |
| [`data/rules/scales.tsv`](data/rules/scales.tsv) | 万 / 億 / 兆 / 京 / 垓… 大数スケール |
| [`data/rules/units.tsv`](data/rules/units.tsv) | SI 単位 (km / kg / mL …) |
| [`data/rules/symbols.tsv`](data/rules/symbols.tsv) | 記号読み (+ / − / % / ‰ …) |
| [`data/rules/latin.tsv`](data/rules/latin.tsv) | ラテン文字読み (A→エー…) |
| [`data/rules/numeric_phrases.tsv`](data/rules/numeric_phrases.tsv) | 例外語句 (二十歳→ハタチ 等) |
| [`data/rules/context.toml`](data/rules/context.toml) | 前後トークンを見る文脈ルール (一日→ツイタチ/イチニチ) |
| [`data/rules/compat_map.tsv`](data/rules/compat_map.tsv) | 異体字 → 標準字 の正規化 (髙→高 等) |

これらは **ビルド時に lib に embed** されるため、`Furigana::minimal()` のみで全機能が動きます。
別ファイルから読み込む場合は `FuriganaBuilder::rules_dir(path)` で上書き可能。

## HTTP API

### `GET /healthz`
```json
{"status": "ok", "dict_size": 0}
```

### `GET /furigana?text=灰桜の道&format=ruby`
```json
{
  "text": "灰桜の道",
  "reading": "{灰桜|はいざくら}の{道|みち}",
  "format": "ruby"
}
```

`format` は `ruby` (default) または `hiragana`。

### `POST /furigana`
```sh
curl -X POST http://127.0.0.1:8000/furigana \
  -H 'Content-Type: application/json' \
  -d '{"text":"灰桜の道","format":"ruby"}'
```

### Bearer 認証
`config.toml` の `[auth].tokens` または起動時 `--token` (env `FURIGANA_TOKEN`) で
1 つ以上のトークンを設定すると `/furigana` で `Authorization: Bearer <token>` 必須。
`/healthz` は常に認証不要。

## 設定ファイル

`~/.config/furigana/config.toml` (Linux/macOS) — 全項目 optional:

```toml
[server]
bind = "127.0.0.1:8000"
cors_origins = []  # 空 = Any 許可 (ローカル用途)

[auth]
tokens = []  # 空 = 認証無効

[rate_limit]
enabled = false
requests_per_min = 600
```

## アーキテクチャ概要

```
crates/
├── furigana/         # lib crate (cargo add furigana)
│   ├── analyzer.rs   # Lindera + IPADIC (形態素解析)
│   ├── kana.rs       # ひら⇄カタ + 正規化
│   ├── numbers.rs    # 数値 → カタカナ + 助数詞ルール (data-driven)
│   ├── reading.rs    # 読み解決パイプライン (top-level)
│   ├── dict.rs       # 単純 surface→reading 辞書 (HashMap ベース)
│   ├── rules/        # データスキーマ (CounterRule / ContextRule 等)
│   ├── loader.rs     # TOML / TSV パーサ
│   ├── embedded.rs   # data/rules/* を build 時に include_str!
│   └── lib.rs        # Furigana 構造体 + builder
└── furigana-cli/     # bin crate (`furigana` バイナリ)
    └── src/
        ├── main.rs            # clap dispatch
        ├── paths.rs           # XDG / %LOCALAPPDATA% 解決
        ├── config.rs          # config.toml ロード
        └── commands/
            ├── lookup.rs      # furigana lookup
            ├── serve.rs       # furigana serve (Axum)
            └── dict.rs        # furigana dict {add,list,remove,import,pull}
```

## ステータスとロードマップ

**Phase 1 (進行中)**: pre-alpha
- ✅ workspace + lib + CLI + データ駆動ルール
- ✅ HTTP server (Axum)
- ✅ 辞書管理コマンド
- 🟡 数値テキスト全体オーケストレーション (`split_num_chunks`)
- 🟡 GitHub Release ワークフロー (binary + Docker image)

**Phase 2 (予定)**:
- 配布用語彙辞書リポジトリ (`furigana-dict`) — `furigana dict pull` で取得
- 単漢字フォールバック (Unihan データの取り込み)
- 辞書のホットリロード (`SIGHUP` / `POST /admin/reload`)

**Phase 3 (検討)**:
- ローマ字出力モード
- 速度最適化 (regex pre-compile pool 等)
- Web Assembly ビルド

## ライセンス

[MIT License](LICENSE)。

## コントリビュート

新しい読みやルール修正は、ほとんどの場合 `data/rules/` 配下の TSV / TOML を編集するだけです。
Rust を書く必要はありません。
詳細は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。
