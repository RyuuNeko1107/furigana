# ja-furigana

[![CI](https://github.com/RyuuNeko1107/ja-furigana/actions/workflows/ci.yml/badge.svg)](https://github.com/RyuuNeko1107/ja-furigana/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/ja-furigana.svg)](https://crates.io/crates/ja-furigana)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/rust-1.88+-orange.svg)](https://www.rust-lang.org)

> A **data-driven local furigana / TTS-prep engine** for Japanese, in Rust.
> Not a complete reading-inference engine — see [立ち位置と精度の限界](#立ち位置と精度の限界).

日本語テキストに **フリガナ (読み仮名 / ルビ)** を付けるための Rust 製ライブラリ + ローカル HTTP サーバー。
形態素解析 (Lindera + IPADIC) と TOML 辞書・ルールを組み合わせた **決定論的** なエンジンで、
TTS 音声合成の前段やふりがな補助での使用を想定しています。

## 立ち位置と精度の限界

このプロジェクトは **「完全な日本語読み推定エンジン」ではありません**。次のような位置付けです:

- ✅ **データ駆動のローカルふりがな / TTS 補助**:
  - VOICEVOX / OpenAI TTS 等の前段で「漢字を含む文 → ひらがな」を一括変換
  - Web / ブログ記事の `<ruby>` タグ自動生成
  - 配信テロップ用の難読語チェック
  - DB の人名・地名フィールドに読みフリガナを付与
- ❌ 苦手なこと (期待しないでほしいこと):
  - **超高精度な文脈読み分け**: 機械学習ベース (BERT 等) のニューラル推論はしません
  - **辞書にない人名・固有名詞**: 形態素解析だけでは自然な読みにならないので、`furigana-dict` の手動 PR で語彙拡充が前提
  - **古文 / 文語 / 方言**: IPADIC ベースなので現代語が中心
  - **同形異音語の完璧な解決**: `rules/context/*.toml` でカバーする範囲は限定的、辞書 PR で個別対応

「不確かなときは形態素解析の素朴な結果に fall back する」「辞書 hit したものは確実に固定する」
という **保守的な決定論** を選んでいます。コミュニティが PR で辞書を拡充するほど精度が上がる設計です。

> **Status**: v0.1.x (alpha)。Phase 1/2 機能は動作するが、**`0.1.x` の間は以下が予告なく変更され得ます**:
> - 公開 Rust API (`Furigana` / `FuriganaBuilder` のメソッドシグネチャ)
> - `furigana-dict` の TOML スキーマ (新フィールド追加、廃止)
> - CLI 引数の名前 / デフォルト値
> - HTTP レスポンスの JSON フィールド名 / 構造
>
> 安定版 (0.1.0 正式) 以降は SemVer で互換を守ります。Rust toolchain は **1.88+** が必要。

## 名前の対応 (混乱しやすい点)

歴史的経緯により、crate 名 / import 名 / バイナリ名がそれぞれ違います:

| 場面 | 名前 | 補足 |
|---|---|---|
| **crates.io の lib crate** | **`ja-furigana`** | `cargo add ja-furigana` で取得 |
| **lib の import 名** | **`ja_furigana`** | `use ja_furigana::Furigana;` (Rust 慣例で `-` → `_`) |
| **crates.io の CLI crate** | **`ja-furigana-cli`** | `cargo install ja-furigana-cli` で導入 |
| **インストール後のバイナリ名** | **`furigana`** | `furigana lookup ...` で実行 (旧来の慣れた名前で残置) |
| **GitHub repo (本体)** | **`RyuuNeko1107/ja-furigana`** | このリポジトリ |
| **GitHub repo (辞書)** | **`RyuuNeko1107/ja-furigana-dict`** | 辞書 PR はここに |

`furigana` という crate 名は別 OSS に取られていたため、`ja-` prefix 付きで OSS 公開しています。
バイナリ名だけは `furigana` のまま (打ちやすさ優先)。

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

- **ライブラリ** (`ja-furigana` crate): `cargo add ja-furigana` で組み込める (import 名は `furigana`)。DB 不要、async 不要、Pure Rust。
- **CLI / ローカル サーバー** (`ja-furigana-cli` → `furigana` バイナリ): ローカル HTTP API + 辞書管理コマンド。

**ローカル利用 / 組み込み用途** を前提に設計。デフォルト bind は `127.0.0.1:8000`、
`/furigana` 認証は無効 (token 設定で有効化)、レート制限なし、ホットリロード対応。

## OSS 化の動機

辞書管理をコミュニティに分担してもらうため。
ルール・辞書はすべて TOML テキストファイルなので、Rust 知識なしで PR を投げられる (再コンパイル不要)。
語彙辞書本体は別リポジトリ [`furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) で受け付け、
`furigana dict pull` で取得する。

## クイックスタート

### ライブラリとして使う

`Cargo.toml`:

```toml
[dependencies]
# crates.io 上の crate 名は `ja-furigana` (`furigana` は別 crate で取られているため)。
# 本リポジトリのライブラリを使うには:
ja-furigana = "0.1.0-alpha.2"
```

```rust
use ja_furigana::Furigana;  // import 名は ja_furigana

let mut f = Furigana::minimal()?;
f.add_reading("灰桜", "ハイザクラ");

println!("{}", f.to_ruby("灰桜の散る道"));
// → "{灰桜|はいざくら}の{散る|ちる}{道|みち}"

println!("{}", f.to_hiragana("灰桜の散る道"));
// → "はいざくらのちるみち"
```

#### `Furigana::minimal()` で何が動いて、何が動かないか

`minimal()` は **空 default** で起動します (辞書ファイルやルール TOML を読まない)。
それでも次は動きます:

- ✅ Lindera (IPADIC) による形態素解析の素朴な読み (動詞・形容詞・既知の名詞)
- ✅ `add_reading()` で動的に追加した語彙の上書き
- ✅ `to_ruby` / `to_hiragana` / `to_tts` / `to_romaji` の各出力モード

逆に **動かない / 機能しないもの**:

- ❌ 助数詞ルール (3冊→さんさつ 等の連濁)
- ❌ 文脈ルール (一日→ツイタチ/イチニチ)
- ❌ 大数スケール / SI 単位 / 記号読み
- ❌ 異体字正規化 (髙→高)
- ❌ 慣用語句 (二十歳→ハタチ)

これらを有効化するには [`furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict)
の TOML を mount します (下の builder API 参照、CLI なら自動配置)。

#### builder API で辞書 / ルールを指定

```rust
use ja_furigana::Furigana;

// 推奨: furigana-dict の中身を `data/` 1 階層に展開した場合
//       core_dict_dir と rules_dir を同じ path にして両方を読ませる
//       (loader は内部で必要なファイルだけ拾う)
let f = Furigana::builder()
    .core_dict_dir("/path/to/data")
    .rules_dir("/path/to/data")
    .user_dict_dir("/path/to/data/user")
    .overrides_file("/path/to/data/overrides.toml")
    .build()?;
```

サンプル: [`crates/furigana/examples/basic.rs`](crates/furigana/examples/basic.rs)

```sh
$ cargo run -p ja-furigana --example basic
```

### CLI として使う

インストール:

```sh
# 一番楽 (Windows): GitHub Releases から furigana-vX.Y.Z-x86_64-pc-windows-msvc.zip を
#   ダウンロードして解凍 → 中の furigana.exe をダブルクリック
#   → 黒い画面 + REPL が立ち上がるので :pull Enter で辞書を取得して試せる
#   https://github.com/RyuuNeko1107/ja-furigana/releases

# Linux / macOS: tar.gz を解凍して PATH のどこかに置く
$ tar -xzf furigana-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
$ mv furigana ~/.local/bin/

# crates.io 経由 (`furigana` バイナリが ~/.cargo/bin にインストールされる)
$ cargo install ja-furigana-cli

# Docker
# 注意: コンテナ内で `furigana serve` の bind を 0.0.0.0:8000 にしないと外から
# 見えません。Docker image の起動コマンドは 0.0.0.0 前提で配置してあります。
# 自分で `docker run ... furigana serve` を呼ぶ場合は `--bind 0.0.0.0:8000` を付ける。
$ docker run --rm -p 8000:8000 ghcr.io/ryuuneko1107/furigana:latest
$ docker run --rm -p 8000:8000 ghcr.io/ryuuneko1107/furigana:latest \
    furigana serve --bind 0.0.0.0:8000
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

# 辞書追加 (default は exe 横の data/user/、`--data-dir` で変更可)
$ furigana dict add 灰桜 ハイザクラ
追加: 灰桜 → ハイザクラ
保存先: <exe と同じフォルダ>/data/user/cli-added.toml

# 辞書反映後に再変換
$ furigana lookup '灰桜の散る道'
{灰桜|はいざくら}の{散る|ちる}{道|みち}

# サーバー起動
$ furigana serve
INFO furigana serving on http://127.0.0.1:8000
INFO 認証 (/furigana): 無効 (ローカル想定)
INFO admin (/admin/reload): 無効 ([auth].admin_tokens を設定すると有効化)

# 対話モード (REPL) — 引数なしで起動 (Windows なら exe ダブルクリック相当)
$ furigana
furigana REPL  (dict_size: 0)
  Tab で補完 / ↑↓ で履歴 / `help` でコマンド / `quit` で終了 (`:` は optional)

辞書が未配置です。furigana-dict (~226 KB) を取得して使えるようにしますか？
[Y/n] > y
最新リリースを確認中...
取得対象: v0.1.1
…
pull + reload 完了。dict_size: 44354

all> 灰桜の散る道
  ruby:     {灰桜|はいざくら}の{散る|ちる}{道|みち}
  hiragana: はいざくらのちるみち

all> mode tts
  mode -> tts

tts> 今日は良い天気ですね、いかがですか？
  きょうはよいてんきですね、   いかがですか?

tts> tokens 灰桜の散る道
  surface  reading
  -------  -------
  灰桜       ハイザクラ
  の        (none)
  散る       チル
  道        みち

tts> quit
```

## 辞書の置き場所

default は **実行ファイルと同じディレクトリ** (portable 構成)。
zip を解凍したフォルダで `furigana.exe` を起動 → `:pull` すると、その横に `data/` が展開される。
exe + `data/` の 2 つだけが見える状態でフォルダごとコピーすれば持ち運び可能。

```
<furigana.exe と同じフォルダ>/
├── furigana.exe                   # 本体
├── config.toml                    # 設定 (任意)
├── repl_history                   # REPL の入力履歴 (自動)
└── data/                          # :pull で展開 / ユーザー追加もここに集約
    ├── unihan.toml                # 単漢字フォールバック
    ├── compat.toml                # 異体字マップ
    ├── jukugo/*.toml              # 熟語 / 固有名詞 / 地名 / 人名
    ├── days.toml                  # 1〜31 日特殊読み
    ├── scales.toml                # 万 / 億 / 兆 ...
    ├── units.toml                 # SI 単位
    ├── symbols.toml               # 記号読み
    ├── latin.toml                 # ラテン文字読み
    ├── numeric_phrases.toml       # 慣用語句 (二十歳→ハタチ等)
    ├── counters/*.toml            # 助数詞ルール
    ├── context/*.toml             # 文脈ルール
    ├── user/                      # ユーザー追加 (`furigana dict add` で生成)
    └── overrides.toml              # 強制上書き用 (最優先、任意)
```

すべて `data/` 直下に flat に並ぶ (旧バージョンの `core/` と `rules/` の分離は廃止)。
内部的に lib loader が「dict 用 (`[entries]` 持つもの) と rules 用 (特定ファイル名)」を
排他的に拾うため、ファイル衝突は発生しない。

`--data-dir <path>` または `FURIGANA_DATA_DIR` 環境変数で別の場所を指定できる
(`cargo install` した場合に `~/.local/share/furigana/` に置きたい等)。

優先順位 (高→低):

1. `data/overrides.toml` (`FuriganaBuilder::overrides_file()`)
2. `data/user/*.toml` (`FuriganaBuilder::user_dict_dir()`)
3. `data/*.toml` + `data/jukugo/*.toml` (`FuriganaBuilder::core_dict_dir()`)
4. 文脈ルール (`data/context/*.toml`、`FuriganaBuilder::rules_dir()`)
5. Lindera (形態素解析) の読み
6. 何もなければ読みなし (`None`) — 出力では surface のまま

## ルール一覧 (全部データ)

| ファイル | 内容 |
|---|---|
| [`furigana-dict/rules/counters/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/rules/counters) | 助数詞 (本/匹/個/年/月/日…) の連濁・促音化・kana 末尾置換 — simple / time / objects 等 7 ファイルに細分化 |
| [`furigana-dict/rules/days.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/days.toml) | 1〜31 日の特殊読み (1→ツイタチ 等) |
| [`furigana-dict/rules/scales.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/scales.toml) | 万 / 億 / 兆 / 京 / 垓… 大数スケール |
| [`furigana-dict/rules/units.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/units.toml) | SI 単位 (km / kg / mL …) |
| [`furigana-dict/rules/symbols.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/symbols.toml) | 記号読み (+ / − / % / ‰ …) |
| [`furigana-dict/rules/latin.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/latin.toml) | ラテン文字読み (A→エー…) |
| [`furigana-dict/rules/numeric_phrases.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/rules/numeric_phrases.toml) | 例外語句 (二十歳→ハタチ 等) |
| [`furigana-dict/rules/context/`](https://github.com/RyuuNeko1107/ja-furigana-dict/tree/master/rules/context) | 前後トークンを見る文脈ルール (一日→ツイタチ/イチニチ) — numbers / homonyms / special の 3 ファイルに細分化 |
| (異体字マップは [`furigana-dict/core/compat.toml`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/core/compat.toml)) | 役割分離のため別リポジトリで管理 |

これらは [`furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) リポジトリで管理され、
`furigana dict pull` で取得 → builder の `rules_dir(path)` で mount する。
本体バイナリには embed しない (バイナリ肥大化を避けるため)。
未配置の状態で `Furigana::minimal()` を呼ぶと空 default で起動し、
助数詞・文脈読み等は無効になるが、形態素解析 (Lindera) と直接 `add_reading` は動作する。

## HTTP API

本番 [ryuuneko.com のフリガナ API](https://ryuuneko.com/?slug=furigana-api) と互換のインターフェース。

### `GET /healthz`
```json
{"status": "ok", "dict_size": 44354}
```

### `GET /furigana?text=灰桜の道&mode=ruby`
```json
{
  "result": "{灰桜|はいざくら}の{道|みち}",
  "mode": "ruby"
}
```

`mode` は `tts` (default) | `hiragana` | `ruby` | `kanji` の 4 つ。
`text` の代わりに `text_b64` (URL-safe base64) でも受ける。

### `POST /furigana`
```sh
curl -X POST http://127.0.0.1:8000/furigana \
  -H 'Content-Type: application/json' \
  -d '{"text":"灰桜の道","mode":"ruby"}'
```

### エラー例

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
// (本文なし、status のみ)

// 503 Service Unavailable — `/admin/reload` で `[auth].admin_tokens` 未設定
// (本文なし、admin 機能 off の合図)
```

`mode` に未知の値を指定した場合は **silently `tts` (default) にフォールバック**します
(本番 ryuuneko.com API と同じ挙動、エラーにはなりません)。

辞書未配置 (`<data_dir>/data/` がまだ無い) の状態で `furigana serve` を起動した場合は、
形態素解析だけは動くので 200 で結果を返しますが、熟語 hit なし・助数詞ルール無効・
文脈ルール無効の degraded mode になります。`/healthz` の `dict_size` が 0 ならこの状態です。

### 他言語クライアント例

`furigana serve` は普通の HTTP API なので、HTTP が話せる言語ならどこからでも使えます。
`examples/clients/` に最小サンプル:

- [Python (`requests`)](./examples/clients/python/example.py) — TTS パイプライン / NLP 系
- [Node.js (組込 `fetch`)](./examples/clients/nodejs/example.mjs) — Discord bot / Web フロント
- [curl + bash](./examples/clients/curl/example.sh) — shell パイプ / 動作確認用

C++ / C# / Go / Ruby などは上の例を参考に好きな HTTP クライアントで。

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
├── furigana/                 # lib crate (crates.io 上の名前は ja-furigana)
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
└── furigana-cli/             # bin crate (crates.io: ja-furigana-cli、binary は furigana)
    └── src/
        ├── main.rs           # clap dispatch (引数なしは repl にフォールバック)
        ├── paths.rs          # 実行ファイル横を default、--data-dir / FURIGANA_DATA_DIR で上書き
        ├── config.rs         # config.toml ロード ([server] / [auth].tokens / .admin_tokens)
        └── commands/
            ├── lookup.rs     # furigana lookup
            ├── repl.rs       # furigana repl (rustyline + Tab 補完 + 履歴)
            ├── dict.rs       # furigana dict {add,list,remove,import,pull}
            ├── dict_pull.rs  #   pull 実装 (GitHub Releases + SHA-256 検証 + tar 展開)
            └── serve/        # furigana serve (Axum HTTP)
                ├── mod.rs    #   run() + Args + shutdown_signal + SIGHUP reload
                ├── handlers.rs # /furigana / /healthz / /admin/reload + do_reload
                ├── auth.rs   #   X-API-Key / Bearer middleware (一般 + admin) + CORS
                └── types.rs  #   FuriganaParams / FuriganaResponse / AppState
```

## ステータスとロードマップ

**Phase 1 (pre-alpha) — 完了**:
- ✅ workspace + lib + CLI + データ駆動ルール (全 TOML)
- ✅ HTTP server (Axum、本番 API 互換)
- ✅ 辞書管理コマンド
- ✅ GitHub Release ワークフロー (5 platform binary + Docker image)
- ✅ 数値テキスト全体オーケストレーション (NumberChunker)
- ✅ [`furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) リポジトリ開設

**Phase 2 — ほぼ完了**:
- ✅ 本番 ryuuneko.com から `furigana-dict` への辞書 seed 投入 (unihan 43,749 / jukugo 605 / compat 436)
- ✅ `furigana dict pull` の実装 (GitHub Releases から tarball を fetch + SHA-256 検証 + 展開)
- ✅ 単漢字フォールバック (Unihan の取り込みは `furigana-dict` 側 seed で達成)
- ✅ 辞書のホットリロード (`SIGHUP` / `POST /admin/reload`)
- ✅ portable 配置 (`furigana.exe` 横に `data/` 1 階層集約)
- ✅ 対話 REPL (`furigana repl` / 引数なし起動 / Tab 補完 / 履歴 / `:` optional)
- ✅ SI 単位の case-insensitive lookup (`1km` / `1KM` / `1Km` 全部 hit)
- ✅ 四字熟語の分離 (`furigana-dict/core/jukugo/four_char.toml`)
- ✅ crates.io 公開 (`ja-furigana` lib + `ja-furigana-cli` bin、0.1.0-alpha.1)

**Phase 3 (進行中)**:
- ✅ ローマ字出力モード (`--mode romaji` / `--mode romaji-kunrei`、ヘボン式 default)
- ✅ 速度最適化 (Lindera analyzer の lazy init で `--version` 等が ~10x 高速)
- 0.1.0 安定版へ昇格 (alpha → 正式)
- 人名・固有名詞の手動振り分け (機械分類が困難なため、PR で順次) (alpha → 正式)

> 旧 Phase 3 候補だった **WebAssembly ビルド** は一度実装したが、`.wasm` が Lindera +
> IPADIC 込みで 57 MB と重く、Web からは `furigana serve` (HTTP API) で十分という
> 判断で削除した。HTTP API ベースの利用例は [`examples/clients/`](./examples/clients/) を参照。

## 主要依存 (Built with)

形態素解析:
- [`lindera`](https://github.com/lindera-morphology/lindera) (MIT) + [`lindera-ipadic`](https://github.com/lindera-morphology/lindera) — IPADIC 形態素解析辞書を embed (NAIST IPADIC 由来、BSD-3-clause-style)

HTTP server / client:
- [`axum`](https://github.com/tokio-rs/axum) (MIT) — `furigana serve` の HTTP レイヤ
- [`tokio`](https://github.com/tokio-rs/tokio) (MIT) — async runtime
- [`reqwest`](https://github.com/seanmonstar/reqwest) (MIT/Apache-2.0) — `furigana dict pull` の HTTP fetch

CLI:
- [`clap`](https://github.com/clap-rs/clap) (MIT/Apache-2.0) — 引数 parser
- [`rustyline`](https://github.com/kkawakam/rustyline) (MIT) — `furigana repl` の line editor (Tab 補完 / 履歴)

その他:
- [`serde`](https://github.com/serde-rs/serde) + [`toml`](https://github.com/toml-rs/toml) (MIT/Apache-2.0) — TOML 設定 / 辞書 parse
- [`flate2`](https://github.com/rust-lang/flate2-rs) + [`tar`](https://github.com/alexcrichton/tar-rs) (MIT/Apache-2.0) — `dict pull` の tar.gz 展開
- [`sha2`](https://github.com/RustCrypto/hashes) (MIT/Apache-2.0) — `dict pull` の SHA-256 検証
- [`regex`](https://github.com/rust-lang/regex) (MIT/Apache-2.0) — pattern matching

依存全件のライセンス全文は [NOTICE.md](NOTICE.md) を参照
([`cargo-about`](https://github.com/EmbarkStudios/cargo-about) で自動生成、CI で license drift を検知)。

## ライセンス

[MIT License](LICENSE)。本リポジトリのコードのみ。
依存ライブラリ各々のライセンスは [NOTICE.md](NOTICE.md) で保持。

## コントリビュート

新しい読みやルール修正は、ほとんどの場合 `furigana-dict/rules/` 配下の TOML を編集するだけです。
Rust を書く必要はありません。
詳細は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。
