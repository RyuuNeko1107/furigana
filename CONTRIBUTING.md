# Contributing to furigana

このリポジトリは **engine (Rust 実装) 専用** です。

> **読みやルールデータの追加・修正は別 repo**:
> [`ja-furigana-dict/CONTRIBUTING.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/CONTRIBUTING.md) を参照。
> Rust 知識・git clone 不要、 GitHub Web UI で TOML 1 行追加から始められる。

## 1. 何を変更したい?

| 変更したいもの | PR 先 |
|---|---|
| **エンジン本体の改修 (Rust)** | このリポジトリ |
| **新しいルール schema (TOML 構造)** | このリポジトリ (`crates/furigana/src/rules/` モジュール) + furigana-dict 側のデータも合わせて |
| HTTP API の挙動変更 | このリポジトリ (`crates/furigana-cli/src/commands/serve/`) |
| 辞書バグ (誤読 / 読み追加 / 異体字 等) | [`ja-furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) |
| engine bug | このリポジトリ |

## 2. Engine 改修の進め方

### 2-1. ローカル環境

[Rust の stable toolchain](https://rustup.rs/) が必要 (MSRV は `Cargo.toml` の `rust-version` 参照)。

```sh
git clone https://github.com/RyuuNeko1107/ja-furigana
cd ja-furigana

cargo build --all-targets
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ja-furigana --example basic
```

### 2-2. テスト用 fixture

`crates/furigana/tests/fixtures/rules/` に `furigana-dict` のスナップショットコピーがあります。
スキーマ変更を伴う engine PR は、必要に応じてこのコピーも同期してください
(辞書データは `furigana-dict` 側がマスター)。

### 2-3. ルール schema を変更する場合

1. このリポジトリで `crates/furigana/src/rules/<file>.rs` の構造体を更新
2. `crates/furigana/tests/fixtures/rules/<file>.toml` も新フィールドに対応
3. 必要なら engine 側の利用コード (numbers / chunks / reading) も更新
4. **同時に** `furigana-dict` リポジトリに対応 PR を出す (新フィールドを使うデータがあるなら)
5. 両方の PR をリンクし、merge 順序を明示する

### 2-4. モジュール構成

| 場所 | 役割 |
|---|---|
| `src/lib.rs` + `api.rs` | 公開 API (`Furigana` / `FuriganaBuilder`) |
| `src/analyzer.rs` | Lindera ラッパ |
| `src/kana.rs` | ひら⇄カタ + Unicode 正規化 |
| `src/numbers/` | 数値処理 (digit / counter / phrase / extras / helpers)、 `phrase.rs` は jukugo Aho-Corasick super-set check 付き |
| `src/chunks/` | テキスト全体の数値・固有語チャンク分割 (URL/日付/jukugo/loanword/scale/SI/counter/symbols/digit の階層的優先確定) |
| `src/reading/` | 読み解決パイプライン (pipeline / merge / context / output)、 `output.rs` で surface 文字種ごとの reading 表記切替 |
| `src/dict.rs` | 単純 surface→reading 辞書 (jukugo / unihan 内部分離、 再帰 walk から `loanwords/` skip) |
| `src/loanwords.rs` | 外来語 (IT 用語等の英単語) 辞書 (case-fold + 全角→半角 + 完全一致 lookup) |
| `src/tts.rs` | TTS 整形 + segment |
| `src/loader.rs` | TOML 汎用パーサ |
| `src/rules/` | ルールデータ schema |

各 module 内に test がある (`#[cfg(test)] mod tests`)。

## 3. PR ガイドライン

### スコープを小さく

1 PR = 1 トピック。`feat: 新機能 + chore: リファクタ + fix: バグ修正` の
混在 PR ではなく、3 つに分けてください。

### コミットメッセージ

[Conventional Commits](https://www.conventionalcommits.org/) を推奨:

- `feat(numbers): 助数詞「篇」のルール追加`
- `fix(reading): 12時の読み修正 (シニジ → ジュウニジ)`
- `refactor(loader): generic 化で wrapper 削減`
- `docs(readme): クイックスタート例を追加`
- `ci: clippy のオプション強化`

### コードの場合

- `cargo fmt --all` を必ず通す (CI で `--check`)
- `cargo clippy --workspace --all-targets -- -D warnings` がクリーン
- 新規公開 API には rustdoc コメント (日本語可)
- 振る舞い変更を伴う場合 1 件以上のテストを添える

## 4. ステータス

Pre-alpha。設計判断は流動的なので、**API/構造を大きく変える PR は Issue で先に相談**
してください。バグ修正・小さい機能追加は普通に PR で OK。
