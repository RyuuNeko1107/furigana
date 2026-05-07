# ja-furigana

[![CI](https://github.com/RyuuNeko1107/ja-furigana/actions/workflows/ci.yml/badge.svg)](https://github.com/RyuuNeko1107/ja-furigana/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/ja-furigana.svg)](https://crates.io/crates/ja-furigana)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/rust-1.89+-orange.svg)](https://www.rust-lang.org)

> A **data-driven local furigana / TTS-prep engine** for Japanese, in Rust.
> Not a complete reading-inference engine — see [立ち位置と精度の限界](#立ち位置と精度の限界).

日本語テキストに **フリガナ (読み仮名 / ルビ)** を付ける Rust 製ライブラリ + ローカル HTTP サーバー。形態素解析 (Lindera + IPADIC) と TOML 辞書・ルールを組み合わせた **決定論的** エンジン。TTS 音声合成の前段やふりがな補助での使用を想定。

[ryuuneko.com のフリガナ API](https://ryuuneko.com/?slug=furigana-api) の OSS 版として開発。同等のインターフェース (`mode` / `text_b64` / `segmented` / `X-API-Key` 等) を提供するので、既存クライアントからの差し替えやセルフホストに使える。

## 立ち位置と精度の限界

このプロジェクトは **「完全な日本語読み推定エンジン」ではありません**。次のような位置付けです:

- ✅ **データ駆動のローカルふりがな / TTS 補助**:
  - VOICEVOX / OpenAI TTS 等の前段で「漢字を含む文 → ひらがな」を一括変換
  - Web / ブログ記事の `<ruby>` タグ自動生成
  - 配信テロップ用の難読語チェック
  - DB の人名・地名フィールドに読みフリガナを付与
  - **IT 用語の英単語にも対応** (Kubernetes / Docker / TypeScript / PostgreSQL 等を
    `core/loanwords/*.toml` で登録 → chunk 全体を完全一致 lookup、 substring 切断ゼロ)
- ❌ 苦手なこと:
  - **超高精度な文脈読み分け**: 機械学習ベース (BERT 等) のニューラル推論はしません
  - **辞書にない人名・固有名詞**: `furigana-dict` の手動 PR で語彙拡充が前提
  - **古文 / 文語 / 方言**: IPADIC ベースなので現代語が中心
  - **同形異音語の完璧な解決**: `rules/context/*.toml` でカバー範囲は限定的

「不確かなときは形態素解析の素朴な結果に fall back」「辞書 hit したものは確実に固定」という **保守的な決定論**。コミュニティ PR で精度が上がる設計。

> **Status**: alpha (0.1.x)。`context rule → jukugo → Lindera → unihan` の 5 段階優先順位で読み解決パイプラインを実装。
> `chunks/split()` 段階で **jukugo prefix-match** (千本桜 等の固有複合語を先取り) と
> **loanwords 完全一致** (Kubernetes / Docker 等の英単語) を独立階層で持つので、
> 助数詞 / 形態素解析より優先される。
> `furigana serve --auto-pull` および `[auto_update]` config による admin_tokens 不要の辞書自動更新、`core/works/<medium>/<title>.toml` のような作品単位細分化辞書 (loader 全階層再帰) もサポート。
> 辞書 (`ja-furigana-dict`) は jukugo を 24 カテゴリに分類 + `loanwords/` で IT 用語等の英単語を別管理、件数の最新値は dict repo の [STATS.md](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/STATS.md) を参照。
> `0.1.x` の間は公開 API / TOML スキーマ / CLI 引数 / HTTP レスポンス構造が予告なく変わりえます。詳細とロードマップは [docs/ROADMAP.md](./docs/ROADMAP.md)、変更履歴は [CHANGELOG.md](./CHANGELOG.md)。

## 名前の対応 (混乱しやすい点)

歴史的経緯で、crate 名 / import 名 / バイナリ名がそれぞれ違います:

| 場面 | 名前 | 補足 |
|---|---|---|
| **crates.io の lib crate** | **`ja-furigana`** | `cargo add ja-furigana` |
| **lib の import 名 (Rust)** | **`furigana`** | `use furigana::Furigana;` ※`-` → `_` 慣例の例外 |
| **crates.io の CLI crate** | **`ja-furigana-cli`** | `cargo install ja-furigana-cli` |
| **インストール後のバイナリ名** | **`furigana`** | `furigana lookup ...` |
| **GitHub repo (本体)** | [`RyuuNeko1107/ja-furigana`](https://github.com/RyuuNeko1107/ja-furigana) | このリポジトリ |
| **GitHub repo (辞書)** | [`RyuuNeko1107/ja-furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) | 辞書 PR はここに |

`furigana` という crate 名は別 OSS に先取りされていたため `ja-` prefix で公開していますが、`[lib] name = "furigana"` 設定により `use ...` は `furigana` のままです。

## 1 分 Quickstart

### ライブラリとして使う

```toml
# Cargo.toml
[dependencies]
ja-furigana = "0.1.0-alpha.6"
```

```rust
use furigana::Furigana;  // crate 名は ja-furigana だが import 名は furigana

let mut f = Furigana::minimal()?;
f.add_reading("灰桜", "ハイザクラ");
println!("{}", f.to_ruby("灰桜の散る道"));
// → "{灰桜|はいざくら}の{散る|ちる}{道|みち}"
```

辞書 / ルールを mount する場合は builder API。詳細は [docs/ARCHITECTURE.md#公開-api-lib](./docs/ARCHITECTURE.md#公開-api-lib)。

### CLI として使う

```sh
# インストール
cargo install ja-furigana-cli
# あるいは GitHub Releases から OS 別 binary を取得 (Windows なら exe ダブルクリックで REPL)

# 1 ショット変換
furigana lookup '灰桜の散る道'                   # → tts (default)
furigana lookup '灰桜の散る道' --mode ruby       # → {灰桜|はいざくら}...

# 出力ルール: 漢字 → ひらがな、 アルファベット / 数字 / 記号 → カタカナ統一
furigana lookup 'Anthropic の Claude を使う' --mode hiragana
# → アンソロピックのクロードをつかう
furigana lookup 'PostgreSQL 16 で動かす' --mode hiragana
# → ポストグレスキューエルジュウロクでうごかす

# 対話モード (引数なしで起動 = REPL)
furigana
# 中で :pull すれば furigana-dict を取得して dict_size が一気に増える

# HTTP サーバー
furigana serve                                  # http://127.0.0.1:8000
furigana serve --auto-pull                      # 起動時に GitHub Releases から最新辞書を取得
# config.toml に [auto_update] enabled=true / interval="24h" で background 定期更新
# (どちらも admin_tokens 設定不要)

# Docker
docker run --rm -p 8000:8000 ghcr.io/ryuuneko1107/furigana:latest
```

## ドキュメント

| ドキュメント | 内容 |
|---|---|
| [`docs/HTTP_API.md`](./docs/HTTP_API.md) | endpoints / `mode` / エラー応答 / 認証 / hot reload / 他言語クライアント |
| [`docs/DATA_LAYOUT.md`](./docs/DATA_LAYOUT.md) | `<data_dir>` 構成 / 優先順位 / `dict pull` の流れ |
| [`docs/RULES.md`](./docs/RULES.md) | ルール一覧 (counters / context / scales / units …) |
| [`docs/CONFIG.md`](./docs/CONFIG.md) | `config.toml` / 環境変数 / CLI フラグ |
| [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) | crate 構成 / 内部モジュール / 設計判断 |
| [`docs/ROADMAP.md`](./docs/ROADMAP.md) | Phase 1/2/3 計画 (CHANGELOG とは別、未来志向) |
| [`CHANGELOG.md`](./CHANGELOG.md) | 完了履歴 (Keep a Changelog 形式) |
| [`CONTRIBUTING.md`](./CONTRIBUTING.md) | 開発 (Rust 側) PR ガイド |
| [`MAINTAINING.md`](./MAINTAINING.md) | release / publish / yank 手順 (メンテナー向け) |

辞書追加の PR は別 repo: [`ja-furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) (Rust 不要、TOML 1 行追加で完結)。

## ライセンス

[MIT License](LICENSE)。本リポジトリのコードのみ。
依存ライブラリのライセンスは [NOTICE.md](NOTICE.md) で保持
([`cargo-about`](https://github.com/EmbarkStudios/cargo-about) で自動生成、CI で license drift 検知)。

## 主要依存 (Built with)

- 形態素解析: [`lindera`](https://github.com/lindera-morphology/lindera) + IPADIC (NAIST 由来、BSD-3-clause-style)
- HTTP: [`axum`](https://github.com/tokio-rs/axum) / [`tokio`](https://github.com/tokio-rs/tokio) / [`reqwest`](https://github.com/seanmonstar/reqwest)
- CLI: [`clap`](https://github.com/clap-rs/clap) / [`rustyline`](https://github.com/kkawakam/rustyline)
- TOML / archive: [`serde`](https://github.com/serde-rs/serde) / [`toml`](https://github.com/toml-rs/toml) / [`flate2`](https://github.com/rust-lang/flate2-rs) / [`tar`](https://github.com/alexcrichton/tar-rs) / [`sha2`](https://github.com/RustCrypto/hashes)

詳細は [NOTICE.md](NOTICE.md)。

## コントリビュート

新しい読みやルール修正は、ほとんどの場合 [`ja-furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) の TOML を編集するだけです (Rust 不要)。エンジン本体 (Rust) の改修は本リポジトリの [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。
