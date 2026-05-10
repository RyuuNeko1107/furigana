# Proposal: Scoring Engine (candidate-based reading resolution)

**Status**: Draft (2026-05-10)
**Target**: 0.1.0 stable (alpha.10 投入 → alpha.11+ dogfood → 0.1.0 stable)
**Scope**: ルビ振り精度向上のための architecture refactor、 0.1.0 stable で SemVer 開始時点から full new architecture

> 関連: [ROADMAP.md](../ROADMAP.md) / [ARCHITECTURE.md](../ARCHITECTURE.md) / [intonation.md](./intonation.md) (Postponed)

## 1. 動機

ja-furigana の現状 4 層構造 (Lindera + IPADIC + ja-furigana-dict + rules) は priority chain (`context rule → jukugo → Lindera → unihan`) で動いており、 以下の問題がある:

- **同形異音語が dict に登録できない** — 1 surface = 1 reading の制約、 文脈分岐は `rules/context/` の限られた表現のみ
- **未知語の単漢字分解が opaque** — Lindera unihan fallback で 「なぜこの読みが選ばれたか」 が trace できない
- **長文未知語が短い完全一致 entry に切り刻まれる** — priority chain では path 全体の妥当性が見えない
- **dict 改善ループが弱い** — 「期待外れの読み」 を contributor が patch する経路が rule 追加経由のみ

これらを **「答えを持つ辞書 → 候補を出す辞書」** の architecture に転換することで解決する。

## 2. 設計指針

1. **dict は候補を出す、 score / longest match / 文脈条件で 1 つ採用** — 全 layer (単語辞書 / 漢字辞書) が candidate を提出、 Viterbi-like 路径選択で文全体の最良 path を採用
2. **連続値 score を採用しない** — discrete band + longest match + 文脈 hit の **lexicographic 比較** で順位決定、 数値 calibration 沼を回避
3. **既存 backwards compat を維持しない** — 0.x 期間中の breaking change は許容、 旧 format は即 reject、 既存 entry は migration script で機械変換
4. **OSS ローカル完結** — telemetry / 学習機能なし、 改善は corpus regression + 手書き PR のみ
5. **scope はルビ振り精度のみ** — 韻律 / accent / TTS 連携は本 proposal 範囲外 ([intonation.md](./intonation.md) 参照、 復帰条件あり)
6. **実装は精度 + 効率の両立最適化** — corpus pass の精度と performance の効率を能動的に optimize する。 数値 cut 要件は設けないが、 benchmark を obligate (§10.2 / §14 参照)、 dogfood で実害があれば改善対象

## 3. dict format

### 3.1 省略形 (大半の entry)

```toml
[entries]
"魔理沙" = "マリサ"
"紅魔館" = "コウマカン"
"切磋琢磨" = "セッサタクマ"
```

文脈分岐がない entry は既存形式そのまま。 50k+ 既存 entry のうち多数はこの形を維持。

### 3.2 完全形 (文脈分岐が要る entry)

inline 形式:

```toml
[entries]
"魔理沙" = "マリサ"
"上手" = { reading = "ジョウズ", match = [
  { next_eq = "から", reading = "カミテ" },
  { prev_pos = "名詞", next_eq = "に", reading = "ウワテ" },
]}
```

または expanded 形式 (推奨、 多 match block 時に読みやすい):

```toml
[entries]
"魔理沙" = "マリサ"

[entries."上手"]
reading = "ジョウズ"             # default reading

[[entries."上手".match]]
next_eq = "から"                # 上手から → カミテ
reading = "カミテ"

[[entries."上手".match]]
prev_pos = "名詞"
next_eq = "に"                  # 名詞 + 上手に → ウワテ
reading = "ウワテ"
```

- `reading` field は必ず指定 (= match 全 miss 時の default、 必須)
- `[[entries."surface".match]]` sub-table を **複数並べる**、 TOML 出現順で第一 hit 採用
- 内部 deserializer は inline / expanded / 省略形 (string) の 3 形式を untagged enum で受け付ける

### 3.3 matcher vocabulary

| 軸 | prev 側 | next 側 | 値型 |
|---|---|---|---|
| literal 一致 | `prev_eq` | `next_eq` | string |
| literal いずれか | `prev_eq_any` | `next_eq_any` | string array |
| 文字種 | `prev_char_type` | `next_char_type` | "漢字" / "ひらがな" / "カタカナ" / "英数" / "記号" |

**Lindera 品詞 matcher は採用しない** (2026-05-10 確定):
- `prev_pos` / `next_pos` / `prev_pos_any` / `next_pos_any` は **削除**
- 理由: Lindera は長期 vision (1.0+) で完全撤廃路線、 廃止予定の機能を新仕様に組み込まない
- trade-off: 「名詞の後 / 動詞の後」 みたいな汎用条件は表現不可、 literal 列挙 (`prev_eq_any = ["階段", "段", "梯子"]` 等) で代用
- Lindera の **形態素分割 + reading** は継続使用 (band 50 unihan injection、 §4.3)

(将来拡張: `prev_within_N` / `next_within_N` 距離指定、 0.3.0+ 検討)

配列値は **1 行 1 要素** (project 全体方針に従う):

```toml
prev_eq_any = [
  "高校",
  "大学",
  "中学",
]
```

### 3.4 block 内 / block 間 semantics

- **同 `[[match]]` block 内**: 全条件 **AND** (全 hit で match 成立)
- **複数 `[[match]]` block**: TOML 出現順で **第一 hit 採用** (= OR but ordered)
- **どの block も hit なし**: `reading` (default) 採用
- `reading` 不在は parse error

### 3.5 `[[kanji]]` block (新規 first-class)

漢字辞書を 「fallback 」 から 「未知語分解時の候補生成器」 に格上げ。 単漢字毎に default + 文脈 match を持つ:

```toml
[[kanji]]
char = "生"
default = "セイ"

[[kanji.match]]
next_eq = "じる"                # 生じる → ショウジル
reading = "ショウ"

[[kanji.match]]
next_pos = "名詞"
prev_char_type = "ひらがな"      # きの生クリーム → ナマ
reading = "ナマ"

[[kanji.match]]
prev_eq_any = [
  "高校",
  "大学",
  "中学",
]                                # 高校生 → セイ (= default だが明示)
reading = "セイ"

[[kanji.match]]
next_eq_any = [
  "まれ",
  "まれる",
]                                # 生まれる → ウ
reading = "ウ"
```

- `char` field は単漢字 1 文字
- `default` field 必須
- matcher vocabulary は entry と完全同一

格納場所: 既存 `core/single/` を rename して `core/kanji/` (新規 dir)、 format も新 `[[kanji]]` array-of-tables に移行 (§6 migration 参照)。

### 3.6 TOML 形式上の制約 (project 全体方針継承)

- 配列は **1 行 1 要素** (merge conflict 抑制)
- triple-quoted string + serde untagged で複合形式受け付け (既存方針)
- `[meta] role` 駆動 loader (既存方針)
- **`[meta] schema_version` field を新規追加** (★5、 dict version compat 仕組み):

```toml
[meta]
schema_version = "2"      # 0.1.0 から v2 (新 format)、 旧 alpha era は v1 (= field 不在 = 暗黙 v1)
role = "entries"
description = "..."
```

lib 側の挙動:
- v2 dict → 正常動作
- v1 dict (= field 不在 or `"1"`) → **明確 error message** で reject:
  ```
  Error: dict schema version 1 not supported by ja-furigana 0.1.x (expected: 2)
         Migrate dict using `furigana-dict/tools/migrate_v2.py` or upgrade dict to v0.1.0+
  ```
- 0.2.0 以降で schema 拡張する場合、 `schema_version = "3"` 等で bump、 lib 側 supported list に追加

### 3.7 intonation bracket notation の forward compat (0.1.0 から)

[intonation.md](./intonation.md) は 0.2.0 stable target だが、 **dict format としては 0.1.0 から bracket notation を受け入れる**:

```toml
[entries]
"上手" = "ジョ]ウズ"             # 0.1.0 から bracket 付き reading が書ける

[entries."橋"]
reading = "ハ]シ"

[[entries."橋".match]]
prev_eq = "鉄"
reading = "テッキョウ"           # match 候補も bracket 付きで書ける
```

**0.1.0 lib の挙動**:
- 読み込み時に bracket / `/` を **strip**、 reading 部分のみ使用 (= accent 情報は捨てる)
- output (hiragana / ruby / romaji) には bracket は含まれない
- 例: `"ジョ]ウズ"` → 内部 reading = "ジョウズ"、 lib output = "じょうず"

**0.1.0 dict validate.py**:
- bracket 構文 check を入れる (各 phrase 内 `]` 最大 1 個 / 空 phrase 禁止 / mora 範囲整合)
- invalid bracket は parse error
- → 0.1.0 期間中に書かれる bracket 付き entry は format 上正しく蓄積、 0.2.0 で reuse 可能

**0.2.0 lib の挙動** (additive):
- bracket parse 実装、 `Token { reading, accent_phrases }` の `accent_phrases` field に保持
- 既存 `reading` field は維持 (= 0.1.0 互換)
- `--mode=accent` / `--mode=voicevox-aques` 等で accent を出力
- 詳細は [intonation.md](./intonation.md) 参照

## 4. score 設計 (discrete band + lexicographic)

連続値 score を採用せず、 **discrete tuple の lexicographic 比較** で順位決定する。 数値 calibration 沼を回避し、 contributor が予測可能な挙動にする。

### 4.1 score 構造

各 candidate edge が以下の score tuple を持つ:

```rust
pub struct Score {
    pub band:             u16,  // 1000 = 単語辞書 / 100 = 漢字辞書 / 50 = Lindera unihan
    pub length:           u8,   // surface 文字数 (longest match 効果)
    pub match_hits:       u8,   // inline match condition hit 数 (default ≠ match)
    pub boundary_penalty: i16,  // (b)(c) ペナルティ累積 (大きいほど悪)
}
```

### 4.2 比較順 (lexicographic)

path 比較時に以下の順で勝者決定:

1. **band 大** (1000 > 100 > 50)
2. **length 大** (longest match)
3. **match_hits 多** (inline match hit ありが優先)
4. **boundary_penalty 軽** (path 内ペナルティ小さい方)
5. **TOML 出現順** (deterministic tie-break)

連続値の重み tuning は不要。 contributor は順位を 「band → 長さ → match → ペナルティ」 の階層で予測できる。

### 4.3 band 値 (確定)

| layer | band | 説明 |
|---|---|---|
| 単語辞書 (`[entries]`) 完全一致 | 1000 | longest match で path に乗る |
| 単語辞書 inline match hit | 1000 + match_hits 反映 | default より match block を優先 |
| 特殊処理 (数字+助数詞 等の動的合成) | **950** | dict 完全一致なし時に候補生成、 dict 完全一致には常に負ける |
| 漢字辞書 (`[[kanji]]`) | 100 | 単漢字 candidate (default or match) |
| Lindera unihan injection | 50 | dict 漢字辞書がないとき最後の fallback |
| アルファベット passthrough | (path edge) | 正規化後 dict 完全一致 miss 時、 そのまま出力 (band 比較対象外) |

**特殊処理 band 950 の意味** (2026-05-10 確定):
- dict 完全一致原則維持 (= contributor が `"3時間" = "サンジカン"` を意図的に書けば 1000 で常に勝つ)
- dict にない場合のみ動的合成が 950 で path に乗る、 漢字辞書 (100) / Lindera (50) には圧倒的勝利
- 「数字 + 助数詞」 を dict に enumerate しなくても自動処理される

**Lindera unihan injection (band 50) の役割明確化**:
- Lindera の **形態素分割 + reading 取得** を継続使用、 candidate provider として band 50 で path に乗せる
- **品詞は使わない** (matcher 削除済、 §3.3)
- 「dict にない token でも何らかの reading が出る」 fallback 用途
- 1.0+ vision で Lindera 完全撤廃時に band 50 が空く、 dict-only 完結に移行

## 5. boundary mitigation

長文未知語が短い完全一致 entry に切り刻まれる問題への 4 軸 multi-layer defense。

### 5.1 (a) longest match (= length lexicographic 比較)

`§4.2` の length 軸が単独で 「surface 長単独 candidate を優遇」 を実現、 連続 score 化なしで (a) 効果を達成。 surface 4 文字 entry vs surface 2 文字 entry × 2 の path では、 length 軸で前者勝利。

### 5.2 (b) 漢字連続 boundary penalty

- 入力中の **漢字 N 文字連続 region** を抽出 (文字種 detect ベース、 Lindera 不使用)
- region 内部を割る path edge に **base penalty −300** (boundary_penalty に加算)
- 例外: region 内に **完全一致 surface entry** がある場合、 その entry の境界 cross は penalty 免除

### 5.3 (c) 未知語 chunk 強化 penalty

- 漢字 N 文字連続 region (N >= 3) で **完全一致 surface が region 内に皆無**
- region 内部を割る edge に **強化 penalty −600** (base penalty に加重)
- 効果: 「魔館」 単独 path より、 「紅魔館」 chunk 化 + 漢字 fallback path が勝つ

### 5.4 (e) contributor 規律 (別 doc)

architecture では救えない領域は contributor 規律で防ぐ。 `furigana-dict/CONTRIBUTING.md` に明文化:

- 漢字 2 文字以下 entry は 「他語の部分一致になりうるか」 を PR レビュー時に必ず check
- 「○○魔館」 形式の固有名詞は、 prefix 全体 (= 「紅魔館」) で entry 化、 短い suffix 部 (= 「魔館」) を単独登録しない
- 出典明示と同等の重みで PR レビュー基準にする

### 5.5 (d) Lindera 境界尊重を採用しなかった理由

(d) Lindera が 1 形態素と認識した範囲を respect する案は **採用しない**:

- Lindera の誤分割を継承するリスク
- 長期 vision (1.0+ Lindera 撤廃) との整合
- dict のみで挙動が決定論的に予測可能になる方が contributor 体験良い

### 5.6 特殊処理 (cross-cutting) の再設計

「廃止」 ではなく 「scoring-engine と整合する形に再設計」 で、 0.1.0 stable cut までに完成 (2026-05-10 確定):

**保護トークン抽出 (URL / 絵文字)**:
- scoring-engine pipeline の **前段** (= 形態素分割 / candidate 生成より前) で抽出
- 抽出した保護 token は candidate 生成対象外、 path 上で固定 edge として保持 (= 完全透過)
- output 復元時に保護 token を re-insert
- 既存 lib の挙動を継承する形で再設計

**アルファベット (英語) passthrough**:
- 英字 token は **正規化後 dict 完全一致 lookup** を試みる
- 正規化: 全角 → 半角、 case-insensitive lookup (詳細 alpha.10 で確定)
- 完全一致 hit → band 1000 candidate として path に乗る (= dict 適用)
- 完全一致 miss → **passthrough edge** として path に乗る (= 入力 surface = output、 reading 振らない、 band 比較対象外)
- 例: `"APIサーバー"` → 「API」 (dict miss → passthrough) + 「サーバー」 (dict hit → reading 適用)
- 出力 hiragana mode: `"APIさーばー"` (英字部はそのまま)
- 出力 ruby mode: `API<ruby>サーバー<rt>さーばー</rt></ruby>` (英字部に rt 付かない)

**数字 + 助数詞 (`rules/numbers/counters/`)**:
- 数字 chunk + 助数詞 entry の **dynamic surface 合成** logic を scoring-engine の candidate provider として再実装
- 「3時間」 「4個」 等の合成 surface を path 上の特殊 candidate edge として生成
- **band 950** (= dict 完全一致 1000 には負ける、 漢字辞書 100 / Lindera 50 には勝つ)
- contributor が `"3時間" = "サンジカン"` を dict に書けば band 1000 で勝つ (= override 可能)
- 既存 `NumberChunker` 挙動を継承

**漢数字 (`rules/numbers/kansuji.toml`) / 数字読み (`rules/numbers/digits.toml`)**:
- 数字 normalize logic を保護トークン抽出と統合、 candidate provider 前段で処理
- 「123」 → 「ヒャクニジュウサン」 等の合成も candidate edge として生成 (band 950)

**踊り字 「々」 自動展開**:
- 入力前処理で 「々」 を直前漢字に置換 (= 既存 logic 継承)
- scoring-engine への入力時には展開済の状態にする
- ただし output 形式 (ruby 等) によっては 「々」 表記を保持する必要あり、 別 layer で復元

**postprocess (`rules/output/postprocess.toml`)**:
- mode 別 regex 置換 layer は **scoring-engine とは別軸** で維持
- pipeline の **後段** (= candidate path 確定後、 mode 別 output 生成時) で適用
- doc で 「scoring-engine の score / candidate logic とは独立」 を明示

**詳細仕様**: alpha.10 着手時に各特殊処理ごとに具体的設計を確定、 本 proposal は全体方針のみ。

## 6. 既存資源 migration + 辞書完全再編成

**辞書完全再編成 (最大 scope) を 0.1.0 stable cut までに実施** (2026-05-10 確定):

| 既存 | 移行先 | 方式 |
|---|---|---|
| `[entries]` 単純形式 (50k+) | 省略形として残置 + **重複 / 古い / 出典なし entry は purge** | 機械変換 + 人手 PR |
| `rules/context/*.toml` | 各 entry の inline `[[entries."x".match]]` | **migration script** |
| `core/single/*.toml` (単漢字 default reading) | `core/kanji/*.toml` の `[[kanji]]` block (`default` field) | **migration script + dir rename** |
| `SingleOverrides` (Issue #15 限定解) | `[[kanji.match]]` block | **migration script** |
| `rules/numbers/counters/*.toml` (cross-cutting) | scoring-engine pipeline 内の特殊処理として再設計 (§5.6) | logic 再実装 |
| `core/jukugo/` (24 カテゴリ) | **再分類** (人手 PR で sub-dir 構成見直し) | 人手 PR |
| `core/works/` | **再分類** (作品単位 sub-dir 整理) | 人手 PR |
| `core/loanwords/` | そのまま (`[entries]` table 形式) または整理 | 確認後判断 |

**実施フロー**:

1. **alpha.10**: lib 側 scoring-engine 投入 + migration script 実装
2. **alpha.11**: dict 完全再編成 PR series:
   - migration script で機械変換 1 PR
   - entry purge (重複 / 古い / 出典なし) 人手 PR series
   - sub-dir 再構成 人手 PR series
3. **alpha.12〜N**: dogfood + corpus calibration
4. **0.1.0 stable cut**: 全部完成後に SemVer 開始

**旧 format は lib 側で即 parse error**、 deprecation warning は出さない (= 0.1.0 を境に新 format only)。 0.1.0 期間中の dict は新 format only、 dict v0.1.0 tag を coordinated に打つ。

**0.1.0 release は時間優先より完成度優先**、 実時間半年〜1 年規模を想定。

## 7. lib 公開 API

### 7.1 既存 API (output 互換維持)

```rust
let f = Furigana::default();
f.to_hiragana("...");  // 既存挙動、 内部 engine が candidate scoring に切替わるが output 同じ
f.to_ruby("...");
f.to_romaji("...");
f.to_kanji("...");
```

内部 engine が変わるだけで public method は変わらない、 caller の rebuild は不要 (ただし dict version は 0.2.0 同期版にする必要あり)。

### 7.2 新 API: `analyze()` debug (★11 確定)

```rust
let f = Furigana::default();
let result: AnalyzeResult = f.analyze("紅魔館を訪問");
```

**0.1.0 で freeze する型** (Minimal scope、 後で additive 追加 SemVer 互換):

```rust
pub struct AnalyzeResult {
    pub tokens: Vec<Token>,
    pub candidates: Vec<Vec<Candidate>>,
    pub path_indices: Vec<usize>,
    pub boundary_regions: Vec<Range<usize>>,
}

pub struct Token {
    pub surface: String,
    pub reading: String,
    pub range: Range<usize>,           // 入力テキスト内の位置
}

pub struct Candidate {
    pub surface: String,
    pub reading: String,
    pub score: Score,
}

pub struct Score {
    pub band: u16,
    pub length: u8,
    pub match_hits: u8,
    pub boundary_penalty: i16,
}
```

caller が candidate / score / path を inspect、 debug や local の dict 改善判断に使う。 **lib は collect しない** (OSS ローカル完結方針)。

internal state (= source ファイル名 / TOML 行番号 / Lindera 内部 token 等) は **expose しない**、 0.1.x で additive 追加可能だが既存 field 削除は breaking。

### 7.3 段階 rollout (alpha 期間活用)

```rust
let f = Furigana::builder()
    .engine(Engine::Smart)   // candidate scoring (alpha 期間中は experimental)
    .build();
```

**CLI flag freeze** (★12 確定、 minimal):
- `--mode hiragana | ruby | romaji | kanji | tts | analyze` (= 既存 + analyze 新規)
- short flag は `-m` のみ
- **`--engine` flag は 0.1.0 で公開しない** (= Smart default 固定、 alpha 期間中の engine 切替は env var `JA_FURIGANA_ENGINE=smart/strict` で対応)
- 0.1.0 stable 以降は Smart default 固定、 Strict は deprecated (削除は 0.2.0+)

**HTTP server schema freeze** (★13 確定):
- endpoints: `GET /furigana?text=&mode=` / `POST /admin/reload` / `GET /healthz` (既存維持)
- response JSON: 既存形式継承、 `mode=analyze` 時に extra field (`tokens`, `candidates`, `path_indices`, `boundary_regions`) 追加
- engine 切替は server 起動時の env var のみ、 query param で切替不可 (= API surface 縮小)

- **alpha.10**: `Engine::Smart` 投入 (experimental flag)、 default は `Engine::Strict`、 dict format 拡張 + matcher (品詞除く) 実装 + 特殊処理再設計 + bracket forward compat (dict 受け入れ + lib strip) + migration script 実装 + **`tools/diff_engines` 投入** (★6) + **dict 側 doc update** (`SCHEMA.md` / `CONTRIBUTING.md` 同時、 ★7) + **`[meta] schema_version`** field (★5)
- **alpha.10 投入後**: **branch protection 復元** (★9)、 alpha.10 自体は **crates.io publish** する (= scoring-engine 投入の節目で dogfood、 ★8)
- **alpha.11**: dict 完全再編成 PR series (機械変換 + entry purge + sub-dir 再構成)、 crates.io publish しない (GitHub release のみ)
- **alpha.12〜N**: Smart engine bug fix loop + corpus calibration + dogfood + **既存機能 freeze 確認 test** (portable / REPL / SI 単位 / ホットリロード / `furigana dict pull`、 ★16) + **benchmark 整備** (criterion 等、 ★14)、 各 alpha は GitHub release のみ
- **0.1.0-rc1**: corpus pass 100% 確認後、 Smart を default に切替、 最終 sanity check、 **benchmark 結果を CHANGELOG に掲載**
- **0.1.0 stable**: full scoring-engine architecture + Smart default + 完全再編成済 dict + Strict deprecated (CHANGELOG 明記) + **MIGRATION.md finalize** (★17)
- **0.1.x**: dict 漸進拡充 / corpus 増強 / bug fix (additive only)
- **0.2.0+**: `Engine::Strict` 削除、 Lindera 信頼度 (band 50 unihan injection) を再評価、 韻律 / intonation 復帰検討

## 8. 段階移行 timeline

「stable まで時間あり」 + 「stable は 0.1.0」 という前提で、 **alpha 期間を活用して architecture を仕上げてから 0.1.0 stable cut** する。

| version | 内容 |
|---|---|
| **alpha.9** (現) | 既存 priority chain (= Strict engine) |
| **alpha.10** | scoring-engine 投入 (Smart experimental) + dict format 拡張 + matcher (品詞除く) 実装 + 特殊処理再設計 (保護トークン / 数字系 / 踊り字 / postprocess) + `analyze()` debug API + migration script 実装 + bracket forward compat (dict 受け入れ + lib strip) |
| **alpha.11** | dict 完全再編成 PR series (機械変換 + entry purge + sub-dir 再構成) |
| **alpha.12〜N** | Smart engine bug fix loop + corpus calibration + dogfood (実時間 数ヶ月規模) |
| **0.1.0-rc1** | Smart engine を default に切替、 corpus pass 100% 確認、 最終 sanity check |
| **0.1.0 stable** | full scoring-engine architecture + Smart default + 完全再編成済 dict + 文脈依存ルビ振りが確実動作、 bracket notation を dict 側 forward compat 受け入れ (lib は strip / 無視) |
| **0.1.x** | dict 漸進拡充 (bracket 付き entry 蓄積含む) / corpus 増強 / bug fix (additive only、 SemVer 互換維持) |
| **0.2.0 stable** | **intonation 機能 + 辞書側韻律対応** ([intonation.md](./intonation.md) Planned for 0.2.0)、 bracket parse 実装、 `Token` に `accent_phrases` field 追加 (additive)、 **`tts` mode に accent 機能追加** (削除しない)、 `--mode=accent` / `--mode=voicevox-aques` 投入 |
| **0.3.0+** | Strict engine 削除、 Lindera 信頼度再評価、 連濁 / 動詞活用 accent shift 等 |

「alpha.9 を最終 alpha」 とした以前の policy は **再撤回** (前回 intonation Postponed で再有効化したものを、 scoring-engine 0.1.0 入りに伴い再度撤回)。 alpha.10〜N は scoring-engine 仕上げのために必要。

stable cut タイミングは **lib (engine) 側の readiness 駆動**。 dict data の充実度は cut 要件から外し、 0.1.x 漸進 / 別 release cycle に倒す:

**0.1.0 stable の position**: 「文脈依存ルビ振りが **確実に動く** 段階」 で cut する (intonation 等の辞書側韻律対応は 0.2.0 stable target、 [intonation.md](./intonation.md) §0 参照)。

**lib 側 必須要件**:
- Smart engine 実装完成 (bug 解消、 stable 動作)
- 既存 corpus regression (`should_read.toml` 現 108 件) が Smart engine で pass
- **文脈依存ルビ振りの動作 verification**: 全 matcher (`prev/next_eq`, `prev/next_pos`, `prev/next_char_type`, `_any` variants) が test pass + 代表的同形異音語 (上手 / 下る / 行った / 人気 / 一日 / 上下 等) を minimal corpus で pass
- migration script `tools/migrate_v2.py` 完成 (dict 側 maintainer が走らせる準備状態)
- public API freeze (`Furigana` builder / `Dict` / `RulesData` 等)
- `analyze()` API schema freeze (schema_version "1")
- HTTP server response JSON フィールド名 freeze
- 旧 format reject 実装
- matcher vocabulary 完全実装
- (a)(b)(c) penalty 数値 fix
- CHANGELOG / MIGRATION.md / doc 整備

**dict 側は cut 要件外** (0.1.x 漸進):
- `[[kanji]]` default reading の常用漢字制覇度
- 文脈 match data の量
- corpus regression 件数の増強 (= 現 108 件 pass で十分、 増強は 0.1.x)
- 「Smart ≧ Strict in 大規模 corpus」 比較 (dict data 依存)

dict 側は **lib 0.1.0 release と coordinated に 1 回**、 migration commit + v0.1.0 tag を打つ。 以降は dict 独立 release cycle で漸進拡充。

## 9. scope 明示 (本 proposal 範囲外)

以下は **本 proposal (0.1.0 stable) の scope 外**、 別 phase で取り扱う:

**0.2.0 stable target** ([intonation.md](./intonation.md) Planned for 0.2.0):
- **intonation (bracket accent 記法、 アクセント核位置)** — bracket parse 実装、 `Token { accent_phrases }` 追加 (additive)
- **韻律 (prosody) / accent_phrase / 辞書側韻律対応**
- **`--mode=accent` / `--mode=voicevox-aques`** 新規投入
- **`tts` mode に accent 機能を追加** (削除しない、 既存 pause 整形は維持、 0.2.0 で `include_accent` opt-in 拡張)
- **`rules/accent/` 階層** + **`rules/numbers/fractions.toml`**

> 注: 0.1.0 stable では bracket notation を dict 側で **書ける** (forward compat)、 lib は strip / 無視。 詳細 §3.7。

**0.3.0+ 検討**:
- **連濁 / 動詞活用 accent shift / 複合語 deaccenting**
- **VOICEVOX / TTS 連携拡張** (`mode=voicevox-query` 等)
- **`Engine::Strict` 削除**
- **Lindera 信頼度再評価** (band 50 unihan injection の見直し、 完全 opt-in 化検討)
- **user_dict CSV 化** (over-engineering 懸念あり、 実データ次第)

**1.0+ vision**:
- **形態素解析撤廃** (ROADMAP §長期 vision 参照)

## 10. Known Limitations

### 10.1 長文未知語の誤分割は完全には解けない

(b)(c) の boundary penalty で大幅緩和するが、 dict 登録状況によっては誤分割が残る:

- 「○○ + 既知短語 + ○○」 の構造で短語が完全一致してしまうケース
- 漢字 2 文字以下の連続では (c) 強化 penalty が効かない (N>=3 閾値)
- Lindera 知らない領域 + 既知短語混入

→ **改善ループ**:

1. corpus regression (`should_read.toml`) に問題例追加 + 期待 reading 登録
2. dict に長 surface entry を PR で追加 (= 「紅魔館 = コウマカン」 等)
3. 次回 build から長 path が score lexicographic で勝つ
4. `analyze()` debug で local 確認、 PR 起こすかの判断材料

これが OSS ローカル完結方針の dict 育成路線 (= telemetry なし、 手書き PR ループ)。

### 10.2 Smart engine の挙動は initial release で完全予測不能

- score parameter (band 値 / boundary penalty 数値) は 0.2.x で calibrate
- corpus pass 100% を experimental 期間で確立、 default 切替 (0.3.0) は corpus pass 率次第
- contributor から見えにくい挙動変化は `analyze()` debug API で観測可能
- Smart / Strict 同時実行で diff を取る運用を推奨 (`tools/diff_engines.py` 等を将来検討)

### 10.3 `[[kanji]]` block のデータ拡充ペース

常用漢字 2136 字 × 平均 3〜5 reading + 文脈 match の整備は段階的:

- 0.2.0 release 時: 既存 `core/single/` migration で default だけ全字揃う状態
- 0.2.x: 頻出 100 字 → 500 字に文脈 match を accreditate (出典明示 PR)
- 0.3.0+: 常用漢字全カバーに向けて漸進
- Lindera unihan injection (band 50) が 「dict にない読みも fallback で出る」 を保証、 強化前段は段階的でよい

### 10.4 performance benchmark 数値要件は設けない (★14 確定)

Smart engine は Strict より遅い可能性 (= candidate 全列挙コスト)、 ただし:

- alpha.10 〜 N で `criterion` 等 benchmark tool 整備、 alpha.12+ から定期計測
- 0.1.0-rc1 で benchmark 結果を CHANGELOG に掲載 (= caller の判断材料)
- 「Strict 比 X 倍以下」 等の **cut 数値要件は設けない** (= dogfood で実害があれば改善対象、 数値ありきにしない)
- ただし 「実装は精度 + 効率の両立最適化」 (§2-6) の指針で能動的に optimize、 「動けばいい」 ではない

### 10.5 dict release pace は Hybrid (★15 確定)

- lib stable cut と coordinated な major/minor bump は SemVer (`v0.1.0` / `v0.2.0`)
- schema 不変の daily-release / 修正 release は CalVer (`v2026.07.01` 等)
- 「dict は data、 lib は code」 の release pace 違いを許容
- daily-release.yml 再開タイミングは 0.1.0 cut 後 user 判断

### 10.6 漢字以外の文字種の boundary penalty は scope 外 (確定)

(b)(c) boundary penalty は **漢字 N 文字連続のみ** が対象。 カタカナ / ひらがな / 英数 / 記号 等の **漢字以外の文字種連続の自動 chunk preservation logic は scope 外**、 0.2.0 以降も検討しない (2026-05-10 確定):

- 未登録外来語 (例: 「ボイスボックス」)、 未登録ひらがな複合語、 等は Lindera band 50 unihan injection で reading 取得済 (= 何らかの読みは出る)
- これら文字種の chunk preservation logic は実装しない
- 実用上、 漢字以外の連続が誤分割される問題は頻度低、 contributor PR で entry 追加が現実的

**ただし、 dict に entry を登録すれば全文字種が普通に動く** (= 既存仕様、 制限なし):

- `core/loanwords/` 等に `"ボイスボックス" = "ボイスボックス"`、 ひらがな entry も同様 → band 1000 完全一致で勝つ
- entry inline match も全文字種で使える (= 文脈分岐 OK)
- 0.2.0 投入時に intonation bracket notation も全文字種 entry に適用可能 (`"ボイスボックス" = "ボ[イスボックス"` 等)、 forward compat の対象
- → 「自動 chunk 化は漢字連続のみ、 dict 登録すれば全文字種で intonation 含めて使える」 整理

## 11. References

- [ROADMAP.md](../ROADMAP.md) — 段階移行 phase 全体像
- [ARCHITECTURE.md](../ARCHITECTURE.md) — 既存 4 層構造の前提
- [intonation.md](./intonation.md) — 棚上げ済み、 復帰条件: scoring-engine の dict 充実 + Smart engine 安定
- [furigana-dict/CONTRIBUTING.md] — (e) contributor 規律 (新規追加予定)

---

**次のアクション**:

1. 本 proposal レビュー + 確定
2. 0.1.0 stable cut (alpha.9 → 0.1.0)
3. 0.2.0 着手:
   - migration script 実装 (`furigana-dict/tools/migrate_v2.py`)
   - lib 側 Smart engine 実装 (Viterbi candidate scoring + boundary penalty + `analyze()` API)
   - test corpus calibration
   - `core/kanji/` 新設 + `core/single/` 廃止
4. CHANGELOG / MIGRATION.md 整備
