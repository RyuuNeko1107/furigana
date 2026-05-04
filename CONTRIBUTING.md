# Contributing to furigana

このリポジトリは **engine (Rust 実装) 専用** です。
読みやルールデータの追加・修正は [`furigana-dict`](https://github.com/RyuuNeko1107/furigana-dict)
リポジトリにお願いします。

## 1. 何を変更したい?

| 変更したいもの | PR 先 |
|---|---|
| 一般語・固有名詞・人名・地名の読み | **`furigana-dict`** [`core/jukugo.toml`](https://github.com/RyuuNeko1107/furigana-dict/blob/master/core/jukugo.toml) |
| 単漢字フォールバック | 同上 `core/unihan.toml` |
| 異体字 → 標準字 | 同上 `core/compat.toml` |
| 慣用読み (例: 浮世絵→ウキヨエ) | 同上 `rules/numeric_phrases.toml` |
| 助数詞 / 連濁ルール | 同上 `rules/counters.toml` |
| 大数スケール / 単位 / 記号 / 文字 | 同上 `rules/{scales,units,symbols,latin}.toml` |
| 文脈依存読み (一日→ツイタチ/イチニチ) | 同上 `rules/context.toml` |
| **エンジン本体の改修 (Rust)** | **このリポジトリ** |
| **新しいルール schema (TOML 構造)** | **このリポジトリ** (`rules/` モジュール) + furigana-dict 側のデータも合わせて |
| HTTP API の挙動変更 | このリポジトリ (`crates/furigana-cli/src/commands/serve/`) |
| バグ修正 | 影響箇所のリポジトリ (engine bug ならこちら、データ誤りなら furigana-dict) |

## 2. Engine 改修の進め方

### 2-1. ローカル環境

[Rust の stable toolchain](https://rustup.rs/) が必要 (MSRV は `Cargo.toml` の `rust-version` 参照)。

```sh
git clone https://github.com/RyuuNeko1107/furigana
cd furigana

cargo build --all-targets
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p furigana --example basic
```

### 2-2. テスト用 fixture

`crates/furigana/tests/fixtures/rules/` に `furigana-dict` のスナップショットコピーがあります。
スキーマ変更を伴う engine PR は、必要に応じてこのコピーも同期してください
(本番データは `furigana-dict` 側がマスター)。

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
| `src/numbers/` | 数値処理 (digit / counter / phrase / extras / helpers) |
| `src/chunks/` | テキスト全体の数値チャンク分割 |
| `src/reading/` | 読み解決パイプライン (pipeline / merge / context / output) |
| `src/dict.rs` | 単純 surface→reading 辞書 |
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
