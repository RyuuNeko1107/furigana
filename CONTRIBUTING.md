# Contributing to furigana

ほとんどの contribution は TOML を編集するだけで完結する。Rust コードは不要。

## 1. 何を追加したい?

| 追加したいもの | PR 先 |
|---|---|
| 一般語・固有名詞・人名・地名の読み | [`furigana-dict`](https://github.com/RyuuNeko1107/furigana-dict) `core/jukugo.toml` |
| 単漢字フォールバック | 同上 `core/unihan.toml` |
| 異体字 → 標準字 | 同上 `core/compat.toml` |
| 慣用読み (例: 浮世絵→ウキヨエ) | 同上 `rules/numeric_phrases.toml` |
| 助数詞 (例: 篇→ヘン) | 同上 `rules/counters.toml` `[simple]` |
| 連濁・促音化が複雑な助数詞 | 同上 `rules/counters.toml` `[counter."X"]` |
| 大数スケール | 同上 `rules/scales.toml` |
| 単位 | 同上 `rules/units.toml` |
| 記号 | 同上 `rules/symbols.toml` |
| ラテン文字 | 同上 `rules/latin.toml` |
| 文脈で読みが変わる語 | 同上 `rules/context.toml` |
| ↑ 上記すべて | 本体 `furigana` リポジトリには **データを含めない**。すべて `furigana-dict` 側で管理 |
| エンジン本体の改修 (Rust) | このリポジトリ |

データ系の PR は全部 `furigana-dict` に。本リポジトリへの PR は engine 側 (Rust) の修正のみ。

### TOML ファイル形式

全データファイルは TOML 形式で統一されています。データの実体・PR 先は
[`furigana-dict`](https://github.com/RyuuNeko1107/furigana-dict) リポジトリを参照。

**単純な key→value table** (numeric_phrases / symbols / latin):

```toml
# 例: rules/numeric_phrases.toml
[entries]
"二十歳" = "ハタチ"
"明後日" = "アサッテ"
```

**追加メタ情報があるもの** (units の `ci` フラグ等):

```toml
# 例: rules/units.toml
[entries]
"km" = { kana = "キロメートル" }
"L"  = { kana = "リットル", ci = true }
```

**順序が大事なもの** (scales の大→小):

```toml
# 例: rules/scales.toml
[[entry]]
kanji = "万"
kana = "マン"

[[entry]]
kanji = "億"
kana = "オク"
```

**構造化ルール** (counters / context) は
[counters.toml](https://github.com/RyuuNeko1107/furigana-dict/blob/master/rules/counters.toml)
に書き方の例が豊富にあります。基本パターン:

```toml
# 単純サフィックス
[simple]
"円" = "エン"

# 連濁・促音化のあるもの
[counter."本"]
default = "ホン"
[[counter."本".rules]]
last_digit = [1, 6, 8, 0]
suffix = "ポン"
sokuonize = true
[[counter."本".rules]]
last_digit = [3]
suffix = "ボン"
```

読みは **全角カタカナ** で書いてください。

## 2. テスト方法

### Rust 環境セットアップ

[Rust の stable toolchain](https://rustup.rs/) が必要 (Cargo.toml の `rust-version` で MSRV 表示)。

```sh
git clone https://github.com/RyuuNeko1107/furigana
cd furigana

# 全テスト (lib + integration)
cargo test

# 警告ゼロでビルド
cargo build --all-targets

# 整形チェック + lint
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# 動作確認
cargo run -p furigana --example basic
```

### テスト用 fixture

本リポジトリの `crates/furigana/tests/fixtures/rules/` にテスト専用のスナップショット
コピーがあります (実データは `furigana-dict` 側で管理)。ルール側の修正をテストする場合
このコピーも同期してください。

```sh
cargo test --test load_real_data
```

新しい読みを追加したら、できればこの integration test に
1〜2 件 assert を足して PR してください。

## 3. PR ガイドライン

### スコープを小さく

1 PR = 1 トピック。「助数詞 5 件追加 + バグ修正 + リファクタリング」
ではなく、3 つの PR に分けてください。

### コミットメッセージ

[Conventional Commits](https://www.conventionalcommits.org/) 推奨:
- `feat(rules): 助数詞「篇」を追加`
- `fix(numbers): 12時の読み修正 (シニジ → ジュウニジ)`
- `docs(readme): クイックスタート例を追加`

### コードの場合

- `cargo fmt` を必ず通す
- `cargo clippy --all-targets -- -D warnings` がクリーン
- 新規公開 API には rustdoc コメント (日本語可)
- できれば 1 〜数件のテストを添える

## 4. ルール追加時の判断基準

「これ data に置くべき? hardcode すべき?」と迷ったら:

- **data に置く**: 表データ (surface → reading)、特殊例外、利用者が増えたら直したくなりそうなもの
- **コードに残す**: 数学的アルゴリズム (例: 数値 → カタカナの桁分解)、解析の構造 (例: パイプライン順序)

迷ったら GitHub Issue で相談してください。

## 5. ステータス

Pre-alpha。設計判断は流動的なので、大きめの変更は Issue で先に相談してください。
小さい辞書追加・ルール修正は普通に PR で OK。
