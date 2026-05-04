# Contributing to furigana

`furigana` は **データ駆動** が基本方針なので、多くの contribution は
**Rust コードを書かずに** TSV / TOML を編集するだけで完結します。
気軽に PR を投げてください。

## 1. 読みを追加したい (一番多いケース)

語彙辞書 (`furigana-dict` リポジトリ — 準備中) は今後別管理になりますが、
本リポジトリ内の以下のルール表は今すぐ PR を受け付けます。

| 追加したいもの | 編集するファイル |
|---|---|
| 慣用読み (例: 浮世絵→ウキヨエ) | [`data/rules/numeric_phrases.tsv`](data/rules/numeric_phrases.tsv) など |
| 助数詞 (例: 篇→ヘン) | [`data/rules/counters.toml`](data/rules/counters.toml) `[simple]` |
| 連濁・促音化が複雑な助数詞 | [`data/rules/counters.toml`](data/rules/counters.toml) `[counter."X"]` |
| 大数スケール | [`data/rules/scales.tsv`](data/rules/scales.tsv) |
| 単位 | [`data/rules/units.tsv`](data/rules/units.tsv) |
| 異体字 → 標準字 | [`data/rules/compat_map.tsv`](data/rules/compat_map.tsv) |
| 文脈で読みが変わる語 | [`data/rules/context.toml`](data/rules/context.toml) |

### TSV ファイル形式

- フィールド区切り: **タブ** (`\t`)
- 行コメント: 行頭 `#`
- 空行は無視
- 読みは **全角カタカナ** で書く

例 (`numeric_phrases.tsv`):
```tsv
# 数字を含む慣用語句
二十歳	ハタチ
明後日	アサッテ
```

### TOML ファイル形式

[counters.toml](data/rules/counters.toml) に書き方の例が豊富にあります。
基本的なパターン:

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

## 2. テスト方法

### Rust 環境セットアップ

[Rust 1.86+](https://rustup.rs/) が必要。

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

### 編集したルールが load できるか確認

`crates/furigana/tests/load_real_data.rs` の integration test が
`data/rules/` 全ファイルをロードして主要エントリを確認します。

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

Pre-alpha のため、設計判断は流動的です。Issue で方針を議論しつつ進めています。

- アーキテクチャに関する議論: GitHub Issues の `architecture` ラベル
- 辞書の追加 / ルール修正: 普通の PR で OK

ありがとうございます 🍀
