# MIGRATION

`ja-furigana` lib + `ja-furigana-cli` bin の major breaking 変更ガイド。
各 release で 「何が壊れたか」 「どう書き換えるか」 を最小例 + rationale で記載。

詳細な commit-level 変更履歴は [CHANGELOG.md](./CHANGELOG.md)、 設計判断の
根拠は [docs/PROPOSALS/](./docs/PROPOSALS/) を参照。

---

## 0.1.x patch (= 0.1.0 stable cut 後)

**SemVer 互換維持** 期間。 0.1.x patch では breaking change なし、
additive (= field 追加 / 新 method / 新 subcommand) のみ。

API stability policy 詳細: [CHANGELOG.md `[0.1.0]` § API stability policy](./CHANGELOG.md)。

---

## 0.1.0-alpha.9 → 0.1.0 (= alpha era → stable cut)

alpha.10〜.21 の累積 breaking change を 0.1.0 stable cut で 1 回適用。
crates.io publish は alpha 期間中 alpha.9 で停止していたため、 alpha 中の
依存者は 0.1.0 にそのまま jump する想定。

### Engine 選択 API 削除 (= alpha.15)

`Engine` enum と切替 API を全削除、 Smart engine が唯一の path に。

```rust
// 旧 (= alpha.9 以前):
use furigana::{Furigana, Engine};
let f = Furigana::builder().engine(Engine::Strict).build()?;
let result = f.engine();

// 新 (= 0.1.0):
use furigana::Furigana;
let f = Furigana::minimal()?;       // or FuriganaBuilder::new().build()?
// engine 選択 method は廃止、 常に Smart engine
```

環境変数 `JA_FURIGANA_ENGINE` も削除、 設定しても無視される。

### Builder method 削除 (= alpha.10〜.11)

```rust
// 旧:
let f = FuriganaBuilder::new()
    .core_loanwords_dir(p)        // ❌ 削除
    .single_overrides_file(p)     // ❌ 削除
    .build()?;

// 新:
// - core_loanwords_dir: core_dict_dir() に統合 (= core/loanwords/ を載せる)
// - single_overrides_file: [[kanji]] block (= core/kanji/*.toml) に統合
let f = FuriganaBuilder::new()
    .rules_dir(rules_path)
    .core_dict_dir(core_path)     // core/jukugo / unihan / kanji / loanwords / works を含む dir
    .build()?;
```

### 内部 module 削除 (= alpha.10〜.15)

以下 pub module を削除、 import 経路がなくなる:

- `furigana::reading::pipeline` (= 旧 Strict engine 7-step、 Smart engine に置換)
- `furigana::reading::context` (= 旧 context rule、 dict 側 `[[entries].match]` に migration)
- `furigana::chunks` (= 旧 chunker、 Smart engine 内蔵)
- `furigana::loanwords` (= 旧 module、 `AlphabetPassthroughProvider` 内に統合)
- `furigana::single_overrides` (= 旧 Issue #15 解、 `[[kanji]]` block に統合)
- `furigana::numbers::phrase` (= 旧 phrase logic、 `NumberCandidateProvider` 内に統合)

caller がこれらを直接 import していた場合は コンパイルエラー。 通常の `Furigana::to_*`
public API 経由なら影響なし。

### `furigana-diff-engines` bin 削除 (= alpha.15)

Strict vs Smart の diff 計測が無意味化 (= Strict 削除済) のため bin 削除。
`cargo install ja-furigana-cli` で `furigana-diff-engines` は生成されない。

### dict 側 TOML format 変更 (= alpha.10〜.11、 coordinated with `ja-furigana-dict` v0.1.0)

`[meta] schema_version = "2"` 必須化。 dict TOML file 冒頭に必ず:

```toml
[meta]
schema_version = "2"
role = "jukugo"   # role 別 dispatch、 詳細は furigana-dict/docs/SCHEMA.md
```

schema_version 不在の dict file は **parse error** で reject。 旧 format dict は
ja-furigana-dict repo 側で alpha.11 時点に machine-migration 済、 利用者は最新
release tarball を `furigana dict pull` で取得すれば自動対応。

#### 旧 `rules/context/` 廃止

```toml
# 旧 (= rules/context/homonyms.toml 等、 alpha.11 で削除):
[[rule]]
surface = "上手"
next_eq = "から"
reading = "カミテ"

# 新 (= core/jukugo/.../*.toml の inline match):
[entries."上手"]
reading = "ジョウズ"

[[entries."上手".match]]
next_eq = "から"
reading = "カミテ"
```

#### `single_overrides.toml` 廃止

1 字 surface override は `core/kanji/*.toml` の `[[kanji]]` block に統合:

```toml
# 旧 (= core/single_overrides.toml、 alpha.11 で削除):
"生" = "セイ"

# 新 (= core/kanji/overrides.toml):
[[kanji]]
char = "生"
default = "セイ"

[[kanji.match]]
next_eq = "じる"
reading = "ショウ"
```

### `[[kanji]]` block / inline match の matcher 仕様

文脈分岐 reading を declarative に書ける matcher を 0.1.0 で正式採用。 詳細は
`furigana-dict/docs/SCHEMA.md` § matcher vocabulary。 主要 axis:

| axis | 意味 | 値型 |
|---|---|---|
| `next_eq` / `prev_eq` | 次 / 前 token の **完全一致** | string |
| `next_eq_any` / `prev_eq_any` | 候補 array での完全一致 | string array |
| `next_starts` / `next_starts_any` | 次 token の **prefix match** (= 1 char 用) | string / array |
| `prev_ends_any` | 前 token の **末尾文字列 match** | string array |
| `next_char_type` / `prev_char_type` | 次 / 前 の文字種 | "漢字" / "ひらがな" / "カタカナ" / "英数" / "記号" |
| `next2_*` | **1 飛ばし next** (= 「人気が無い」 の 「無」 等) | next_* と同形 |

**注意**: `next_eq` は `next_logical_token` (= 文字種連続範囲) と完全比較する。
1 char マッチしたいときは必ず `next_starts` / `next_starts_any` を使う:

```toml
# ❌ 動かない (= next_eq は 「がいい」 全体と比較、 「が」 1 char match できない):
[[kanji.match]]
next_eq = "が"
reading = "ホウ"

# ✅ 正しい:
[[kanji.match]]
next_starts_any = ["が", "は", "に", "を", "も"]
reading = "ホウ"
```

`next_pos` / `prev_pos` (= Lindera 品詞) は **採用しない**。 Lindera 撤廃路線
(= 0.2.0+ で intonation 計算に UniDic を使うが品詞は読み確定に使わない) と整合。

### 公開関数の signature 変更

#### `Furigana::analyze()` 戻り値型変更

```rust
// 旧 (= alpha.9 以前、 内部 debug 用 unstable):
// (該当 API なし or 別 signature)

// 新 (= 0.1.0、 stable 公開):
pub fn analyze(&self, input: &str) -> AnalyzeResult;

pub struct AnalyzeResult {
    pub tokens: Vec<Token>,
    pub candidates: Vec<Vec<Candidate>>,
    pub path_indices: Vec<usize>,
    pub boundary_regions: Vec<Range<usize>>,
}
```

`AnalyzeResult` / `Token` / `Candidate` / `Score` は **`#[non_exhaustive]`** で
0.2.0+ field 追加余地あり。 caller は literal struct 構築せず lib が return した
値を field access で使う想定。

### bracket notation (= intonation forward compat)

0.1.0 から dict 値に `[` `]` `/` を許可 (= forward compat for 0.2.0)、 lib は
strip / 無視する:

```toml
[entries]
"上手" = "ジョ]ウズ"     # 0.1.0 = lib が strip → "ジョウズ"、 0.2.0+ で accent 利用
"霧雨" = "キ[リサメ"     # 0.1.0 = lib が strip → "キリサメ"
```

`furigana::scoring::bracket::strip_intonation_markers` で internal strip、 通常
caller は意識不要。

---

## 0.2.0 stable target (= intonation 投入、 計画中)

0.2.0 で `[` `]` `/` bracket parse して accent annotation 出力、 `--mode=accent`
追加予定。 詳細は [docs/PROPOSALS/intonation.md](./docs/PROPOSALS/intonation.md)。

予想 breaking:
- `Token` に `pos: Option<String>` 等 accent 関連 field 追加 (= additive、
  `#[non_exhaustive]` でカバー)
- `TtsOptions` variant 追加 (= 同上)
- 既存 0.1.x コードは無修正で動作見込み

---

## 移行の質問 / Issue

不明な点や移行 sample は [GitHub Issues](https://github.com/RyuuNeko1107/ja-furigana/issues)
へ。 0.1.0 cut 後 1 〜 3 ヶ月は alpha 由来の移行サポート期間として運用想定。
