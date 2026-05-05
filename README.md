# furigana

[![CI](https://github.com/RyuuNeko1107/furigana/actions/workflows/ci.yml/badge.svg)](https://github.com/RyuuNeko1107/furigana/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)

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

[ryuuneko.com のフリガナ API](https://ryuuneko.com/?slug=furigana-api) の **OSS 版**。
本番 API と同じインターフェース (`mode` / `text_b64` / `segmented` / `X-API-Key` 等) で動くので、
既存プラグインから差し替えやセルフホストが容易。

提供形態:

- **ライブラリ** (`furigana` crate): `cargo add furigana` で組み込める。DB 不要、async 不要、Pure Rust。
- **CLI / ローカル サーバー** (`furigana-cli` → `furigana` バイナリ): ローカル HTTP API + 辞書管理コマンド。

**ローカル利用 / 組み込み用途** を前提に設計。デフォルト bind は `127.0.0.1`、認証なし、レート制限なし。

## OSS 化の動機

辞書管理をコミュニティに分担してもらうため。
ルール・辞書はすべて TOML テキストファイルなので、Rust 知識なしで PR を投げられる (再コンパイル不要)。
語彙辞書本体は別リポジトリ [`furigana-dict`](https://github.com/RyuuNeko1107/furigana-dict) で受け付け、
`furigana dict pull` で取得する。

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
    .overrides_file("/path/to/overrides.toml")
    .build()?;
```

サンプル: [`crates/furigana/examples/basic.rs`](crates/furigana/examples/basic.rs)

```sh
$ cargo run -p furigana --example basic
```

### CLI として使う

インストール:

```sh
# crates.io 経由 (公開後)
$ cargo install furigana-cli

# GitHub Releases からプリビルド binary
#   https://github.com/RyuuNeko1107/furigana/releases から
#   {furigana-vX.Y.Z-<target>}.{tar.gz | zip} を取得して PATH に置く

# Docker
$ docker run --rm -p 8000:8000 ghcr.io/ryuuneko1107/furigana:latest
```

実行例:

```sh
# 1 ショット変換 (default は tts モード)
$ furigana lookup '明日は9時30分に集合'
あしたは9じ30ふんにしゅうごう

$ furigana lookup '灰桜の散る道' --mode ruby
{灰|はい}{桜|さくら}の{散る|ちる}{道|みち}

$ furigana lookup '灰桜の散る道' --mode hiragana
はいさくらのちるみち

# 辞書追加
$ furigana dict add 灰桜 ハイザクラ
追加: 灰桜 → ハイザクラ
保存先: ~/.local/share/furigana/dict/user/cli-added.toml

# 辞書反映後に再変換
$ furigana lookup '灰桜の散る道'
{灰桜|はいざくら}の{散る|ちる}{道|みち}

# サーバー起動
$ furigana serve
INFO furigana serving on http://127.0.0.1:8000
INFO Bearer 認証: 無効 (ローカル想定)

# 対話モード (REPL) — 手動で試したいとき
$ furigana repl
furigana REPL
  dict_size: 44354
  type :help for commands, Ctrl-D to quit
all> 灰桜の散る道
  ruby:     {灰桜|はいざくら}の{散る|ちる}{道|みち}
  hiragana: はいざくらのちるみち
all> :mode tts
mode -> tts
tts> 今日は良い天気ですね、いかがですか？
きょうはよいてんきですね、 いかがですか?
tts> :tokens 灰桜の散る道
  surface  reading
  -------  -------
  灰桜       ハイザクラ
  の        (none)
  散る       チル
  道        みち
tts> :quit
```

## 辞書の置き場所

```
~/.local/share/furigana/dict/      (Windows: %LOCALAPPDATA%\furigana\dict\)
├── core/                          # `furigana dict pull` で配布版を取得 (準備中)
│   ├── ja_auto.toml
│   └── unihan_kana.toml
├── user/                          # ユーザーが自由に *.toml を置く
│   └── cli-added.toml              # `furigana dict add` の保存先
└── overrides.toml                  # 強制上書き用 (最優先)
```

優先順位 (高→低):

1. `overrides.toml` (FuriganaBuilder の `overrides_file()`)
2. `user/*.toml` (FuriganaBuilder の `user_dict_dir()`)
3. `core/*.toml` (FuriganaBuilder の `core_dict_dir()`)
4. 文脈ルール (`furigana-dict/rules/context/*.toml`)
5. Lindera (形態素解析) の読み
6. 何もなければ読みなし (`None`) — 出力では surface のまま

## ルール一覧 (全部データ)

| ファイル | 内容 |
|---|---|
| [`furigana-dict/rules/counters/`](https://github.com/RyuuNeko1107/furigana-dict/tree/master/rules/counters) | 助数詞 (本/匹/個/年/月/日…) の連濁・促音化・kana 末尾置換 — simple / time / objects 等 7 ファイルに細分化 |
| [`furigana-dict/rules/days.toml`](https://github.com/RyuuNeko1107/furigana-dict/blob/master/rules/days.toml) | 1〜31 日の特殊読み (1→ツイタチ 等) |
| [`furigana-dict/rules/scales.toml`](https://github.com/RyuuNeko1107/furigana-dict/blob/master/rules/scales.toml) | 万 / 億 / 兆 / 京 / 垓… 大数スケール |
| [`furigana-dict/rules/units.toml`](https://github.com/RyuuNeko1107/furigana-dict/blob/master/rules/units.toml) | SI 単位 (km / kg / mL …) |
| [`furigana-dict/rules/symbols.toml`](https://github.com/RyuuNeko1107/furigana-dict/blob/master/rules/symbols.toml) | 記号読み (+ / − / % / ‰ …) |
| [`furigana-dict/rules/latin.toml`](https://github.com/RyuuNeko1107/furigana-dict/blob/master/rules/latin.toml) | ラテン文字読み (A→エー…) |
| [`furigana-dict/rules/numeric_phrases.toml`](https://github.com/RyuuNeko1107/furigana-dict/blob/master/rules/numeric_phrases.toml) | 例外語句 (二十歳→ハタチ 等) |
| [`furigana-dict/rules/context/`](https://github.com/RyuuNeko1107/furigana-dict/tree/master/rules/context) | 前後トークンを見る文脈ルール (一日→ツイタチ/イチニチ) — numbers / homonyms / special の 3 ファイルに細分化 |
| (異体字マップは [`furigana-dict/core/compat.toml`](https://github.com/RyuuNeko1107/furigana-dict/blob/master/core/compat.toml)) | 役割分離のため別リポジトリで管理 |

これらは [`furigana-dict`](https://github.com/RyuuNeko1107/furigana-dict) リポジトリで管理され、
`furigana dict pull` で取得 → builder の `rules_dir(path)` で mount する。
本体バイナリには embed しない (バイナリ肥大化を避けるため)。
未配置の状態で `Furigana::minimal()` を呼ぶと空 default で起動し、
助数詞・文脈読み等は無効になるが、形態素解析 (Lindera) と直接 `add_reading` は動作する。

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
1 つ以上のトークンを設定すると `/furigana` で `X-API-Key` または
`Authorization: Bearer <token>` 必須。
`/healthz` は常に認証不要。

### ホットリロード
辞書を再読込するには:

- `POST /admin/reload` — `[auth].admin_tokens` に登録した token で認証
  (一般 `tokens` では通らない、admin 専用)
- `kill -HUP <pid>` (Unix のみ) — systemd の `ExecReload` に相当

`furigana dict pull` で新版を取得 → `/admin/reload` で反映、というのが想定フロー。
`admin_tokens` が空の場合 `/admin/reload` は **503** を返して機能 off。

## 設定ファイル

`~/.config/furigana/config.toml` (Linux/macOS) — 全項目 optional:

```toml
[server]
bind = "127.0.0.1:8000"
cors_origins = []  # 空 = Any 許可 (ローカル用途)

[auth]
tokens = []        # 空 = /furigana 認証無効 (ローカル想定)
admin_tokens = []  # 空 = /admin/* 機能 off (503)
```

## アーキテクチャ概要

```
crates/
├── furigana/                 # lib crate (cargo add furigana)
│   ├── lib.rs                # module 宣言 + 公開 API re-export (ファサード)
│   ├── api.rs                # Furigana 構造体 + FuriganaBuilder
│   ├── analyzer.rs           # Lindera + IPADIC (形態素解析)
│   ├── kana.rs               # ひら⇄カタ + Unicode 正規化
│   ├── dict.rs               # 単純 surface→reading 辞書 (HashMap)
│   ├── tts.rs                # TTS 整形 + segment
│   ├── error.rs              # FuriganaError / Result
│   ├── loader.rs             # TOML 汎用 parser (parse_toml<T> + load_or_default<T>)
│   ├── embedded.rs           # 空 default RulesData (rules は furigana-dict 側)
│   ├── rules/                # データスキーマ (counters / context / scales / units / ...)
│   ├── numbers/              # 数値処理 (data-driven)
│   │   ├── helpers.rs        #   zen2han / norm_num / sokuonize_last 等
│   │   ├── digit.rs          #   number_to_katakana
│   │   ├── counter.rs        #   euphonic_counter_read
│   │   ├── phrase.rs         #   NumericPhraseMatcher
│   │   └── extras.rs         #   scale/si_unit/symbol 単発読み
│   ├── chunks/               # テキスト全体の数値チャンク分割
│   │   ├── mod.rs            #   NumberChunker + split()
│   │   └── regex.rs          #   静的 / 動的 regex + builder
│   └── reading/              # 読み解決パイプライン
│       ├── mod.rs            #   ReadingToken + tokenize_text (top-level)
│       ├── pipeline.rs       #   tokenize_chunk + resolve_reading
│       ├── merge.rs          #   merge_with_dict (最長一致結合)
│       ├── context.rs        #   apply_context_rules (data-driven)
│       └── output.rs         #   tokens_to_hiragana / tokens_to_ruby
└── furigana-cli/             # bin crate (`furigana` バイナリ)
    └── src/
        ├── main.rs           # clap dispatch
        ├── paths.rs          # XDG / %LOCALAPPDATA% 解決
        ├── config.rs         # config.toml ロード
        └── commands/
            ├── lookup.rs     # furigana lookup
            ├── dict.rs       # furigana dict {add,list,remove,import,pull}
            └── serve/        # furigana serve (Axum HTTP)
                ├── mod.rs    #   run() + Args + shutdown_signal
                ├── handlers.rs #  /furigana / /healthz ハンドラ + 変換
                ├── auth.rs   #   X-API-Key / Bearer middleware + CORS
                └── types.rs  #   FuriganaParams / FuriganaResponse / AppState
```

## ステータスとロードマップ

**Phase 1 (pre-alpha) — 完了**:
- ✅ workspace + lib + CLI + データ駆動ルール (全 TOML)
- ✅ HTTP server (Axum、本番 API 互換)
- ✅ 辞書管理コマンド
- ✅ GitHub Release ワークフロー (5 platform binary + Docker image)
- ✅ 数値テキスト全体オーケストレーション (NumberChunker)
- ✅ [`furigana-dict`](https://github.com/RyuuNeko1107/furigana-dict) リポジトリ開設

**Phase 2 — 進行中**:
- ✅ 本番 ryuuneko.com から `furigana-dict` への辞書 seed 投入 (unihan 43,749 / jukugo 605 / compat 436)
- ✅ `furigana dict pull` の実装 (GitHub Releases から tarball を fetch + SHA-256 検証 + 展開)
- ✅ 単漢字フォールバック (Unihan の取り込みは `furigana-dict` 側 seed で達成)
- ✅ 辞書のホットリロード (`SIGHUP` / `POST /admin/reload`)
- crates.io 公開 (`furigana` lib + `furigana-cli` bin)

**Phase 3 (検討)**:
- ローマ字出力モード
- 速度最適化 (regex pre-compile pool 等)
- Web Assembly ビルド

## ライセンス

[MIT License](LICENSE)。

## コントリビュート

新しい読みやルール修正は、ほとんどの場合 `furigana-dict/rules/` 配下の TOML を編集するだけです。
Rust を書く必要はありません。
詳細は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。
