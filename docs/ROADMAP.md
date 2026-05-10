# ロードマップ

ja-furigana の中長期計画。 **完了履歴は [CHANGELOG.md](../CHANGELOG.md)** を参照。
本書は「これから何をやるか」 志向で書く。

> 戻る: [README](../README.md)

## ステータス概観

**v0.1.x (alpha)**: Phase 1〜5 機能はすべて動作。 alpha.9 を最終 alpha として、
**0.1.0 正式版** を直近目標としている。

`0.1.x` の間は以下が予告なく変更されうる:

- 公開 Rust API (`Furigana` / `FuriganaBuilder` のメソッドシグネチャ)
- `furigana-dict` の TOML スキーマ (新フィールド追加、廃止)
- CLI 引数の名前 / デフォルト値
- HTTP レスポンスの JSON フィールド名 / 構造

安定版 (0.1.0 正式) 以降は SemVer で互換を守る。 Rust toolchain は **1.89+** が必要
(`std::fs::File::lock` 安定化要求のため、 依存 rustyline 18 経由)。

## 完了済み

詳細は [CHANGELOG.md](../CHANGELOG.md) で。 サマリのみ:

### Phase 1〜2 (~2026-05-05)
- workspace + lib + CLI + データ駆動ルール (全 TOML)
- HTTP server (Axum)、 辞書管理コマンド、 GitHub Release ワークフロー
- `furigana-dict` リポジトリ開設 + seed 投入
- `furigana dict pull` (GitHub Releases + SHA-256 検証 + 展開)
- ホットリロード (`SIGHUP` / `POST /admin/reload`)
- portable 配置、 対話 REPL、 SI 単位 case-insensitive、 ローマ字出力モード
- crates.io 公開 (`ja-furigana` lib + `ja-furigana-cli` bin)
- Lindera analyzer の lazy init

### Phase 3 (~2026-05-06、 alpha.3)
- 本番互換の **5 段階優先順位** (`context rule → jukugo → Lindera → unihan`)
- `Dict` の jukugo / unihan 内部分離
- `NumberChunker` の漢数字対応 + scale+unit 連結 + counter context
- `postprocess.toml` (Step 7 mode 別 regex 置換)
- 検証ループ駆動の品質改善基盤 (`should_read.toml` + `tools/run_corpus.py`)
- CI の audit / corpus regression job 追加

### Phase 4 (運用基盤)
- 辞書自動更新 (`--auto-pull` + `[auto_update]`、 admin_tokens 不要)
- `Dict::from_toml_dir` 全階層再帰
- 作品単位辞書 `core/works/` 新設
- 辞書大規模拡充 (jukugo 24 カテゴリ、 4.5k 件超)
- STATS.md 自動生成基盤

### Phase 5 (lookup priority + 外来語 + 出力ルール、 alpha.7)
- jukugo Aho-Corasick prefix-match (chunks 階層 4.5)
- 外来語 (loanwords) 辞書サポート (chunks 階層 4.7、 完全一致 lookup)
- 出力ルール仕様変更 (surface 文字種で reading 表記分岐)
- cross-file 重複検出の自動化 (validate.py + STATS_DUPS.md)
- 踊り字「々」 自動展開
- 単漢字 default override (`SingleOverrides`、 issue #15 限定解)

### Phase 6 (security + role 駆動 loader、 alpha.8〜alpha.9)
- security 全 8 軸補強 (archive 展開 caps / HTTP body limit + rate limit / ReDoS audit /
  RCE audit / sanitize layer for dict load / timing-safe token 比較 /
  GitHub tag strict format / CLI 制御文字 reject)
- `[meta] role` 駆動 loader (rules + dict 両方を統一 dispatch)
- rules 3 sub-dir 階層化 (numbers / context / output)
- inline test (`*.test.toml`) の append-only CI 強制
- dict TOML format DSL 化 (triple-quoted string)
- days.toml の `[entries]` block 化

## 進行中 / 候補

### 0.1.0 stable に向けて

#### Phase 7: Scoring Engine (candidate-based reading resolution) ★ 0.2.0
詳細仕様: [docs/PROPOSALS/scoring-engine.md](./PROPOSALS/scoring-engine.md)

「答えを持つ辞書 → 候補を出す辞書」 への architecture 転換。 ルビ振り精度向上が target、 韻律 / accent / TTS 連携は本 phase scope 外。

**dict 側 (ja-furigana-dict、 0.1.0 stable 同期)**

- [ ] entry inline match notation (`[[entries."x".match]]` sub-table) 受け入れ
- [ ] `[[kanji]]` block first-class candidate generator (`core/kanji/` 新設)
- [ ] `rules/context/` 廃止、 中身は entry inline に migration
- [ ] `core/single/` を `core/kanji/` に rename + format 変換
- [ ] `SingleOverrides` (Issue #15) を `[[kanji.match]]` に統合
- [ ] migration script `tools/migrate_v2.py` 実装、 機械変換 PR 1 本
- [ ] **重複 / 古い / 出典なし entry の purge** (人手 PR series、 alpha.11)
- [ ] **`core/jukugo/` 24 カテゴリ 再分類** (人手 PR series、 alpha.11)
- [ ] **`core/works/` 作品単位 sub-dir 整理** (人手 PR series、 alpha.11)
- [ ] **`core/loanwords/` 整理確認** (alpha.11)
- [ ] validate.py 拡張 (matcher vocabulary check、 `default` 必須 check、 **bracket 構文 check** = forward compat、 **`[meta] schema_version`** 必須 check ★5)
- [ ] reading に bracket notation (`[`, `]`, `/`) を許可 (forward compat、 0.1.0 から書ける)
- [ ] **`SCHEMA.md` 全面 update** (alpha.10 同時、 ★7): 新 entry inline match notation / `[[kanji]]` block / matcher vocabulary / `[meta] schema_version`
- [ ] **`CONTRIBUTING.md` 新規作成** (alpha.10 同時、 ★7): (e) 規律 + 出典明示 + bracket notation 書き方
- [ ] **dict release pace Hybrid** (★15): SemVer (lib coordinated `v0.1.0` / `v0.2.0`) + CalVer (daily-release / 修正)、 daily-release.yml 再開は 0.1.0 cut 後 user 判断

**lib 側 (ja-furigana、 0.1.0 stable)**

- [x] candidate scoring engine 実装 (Viterbi-like path 選択、 **精度 + 効率最適化**)
- [x] discrete band + lexicographic 比較 (連続値 score 不採用)
- [x] entry inline match parser (untagged enum で省略形 / inline / expanded を吸収)
- [x] `[[kanji]]` block parser
- [x] matcher vocabulary 実装 (literal / char_type のみ、 **品詞 matcher は不採用**)
- [x] (b) 漢字連続 boundary penalty + (c) 未知語 chunk 強化 penalty
- [x] (a) longest match (length lexicographic 比較で実現)
- [ ] **特殊処理 (cross-cutting) 再設計実装**: 保護トークン抽出 (URL/絵文字) ✅ / **アルファベット passthrough** ✅ / 数字 + 助数詞 (band 950) / 漢数字 / 数字読み / 踊り字 「々」 自動展開 ✅ (Smart engine post-pass 連濁) / postprocess ✅ (scoring-engine 独立性 doc 明示) (= C1/C2/C4 完了、 C3 残)
- [x] **bracket notation forward compat**: 読み込み時に `[`, `]`, `/` を strip、 reading 部分のみ使用
- [x] `Engine::Smart` (experimental flag、 default は `Engine::Strict`)、 切替は env var `JA_FURIGANA_ENGINE` のみ、 **CLI `--engine` flag は公開しない**
- [x] `Furigana::analyze()` debug API (★11 確定型: AnalyzeResult / Token / Candidate / Score)
- [x] **CLI `--mode analyze` 追加** (analyze 出力 mode、 ★12)
- [x] **HTTP server schema freeze**: 既存 endpoint + `mode=analyze` 時に extra field (★13)
- [ ] 旧 format は parse error (`[meta] schema_version` 検証で v2 以上要求、 ★5) (= validator は実装済、 各 caller への wire-up = A1b は dict v2 化と coordinated 待ち)
- [ ] Lindera 形態素分割 + reading は継続使用 (band 50 unihan injection、 品詞は使わない) (= Smart engine の lindera-unihan provider は alpha.10 段階未統合、 alpha.10〜rc1 で追加予定)
- [x] **`tools/diff_engines` (Smart vs Strict diff) 投入** (★6、 alpha.10 同時)
- [ ] **benchmark 整備** (criterion、 ★14、 alpha.12+)、 数値 cut 要件なし、 0.1.0-rc1 で CHANGELOG 掲載
- [ ] **既存機能 freeze 確認 test** (★16): portable 配置 / REPL / SI 単位 / ホットリロード / `furigana dict pull`
- [ ] CHANGELOG `[Unreleased]` 蓄積 → 0.1.0 cut 時 finalize (★17)
- [ ] **MIGRATION.md 新規** (alpha 期間中蓄積、 0.1.0 cut で finalize、 ★17)

**dict 側 contributor 規律 (`furigana-dict/CONTRIBUTING.md`)**

- [ ] 漢字 2 文字以下 entry の PR レビュー基準明文化 (= (e) 規律)
- [ ] 「○○魔館」 系 suffix 単独登録の禁止
- [ ] 出典明示と同等の重みで規律違反を merge block

#### Phase 8: アクセント (intonation) 機能 — 0.2.0 stable target
詳細仕様: [docs/PROPOSALS/intonation.md](./PROPOSALS/intonation.md) (Status: Planned for 0.2.0 stable)

**dict 側は 0.1.0 から書ける** (forward compat、 lib は strip / 無視)、 0.2.0 で lib が parse して活用。

0.2.0 stable で投入:
- bracket notation parse 実装、 `Token { accent_phrases }` field 追加 (additive、 SemVer minor 互換)
- `--mode=accent` 中立 JSON 出力
- `--mode=voicevox-aques` AquesTalk-風記法
- **`tts` mode に accent 機能を追加** (削除しない、 既存 pause 整形は維持、 `include_accent` opt-in)
- `rules/accent/` 階層 + `rules/numbers/fractions.toml`

詳細は intonation.md §0 / §8 参照。

#### 既存 TODO

- [ ] **公開 API のシグネチャ最終確認** — rename したくないものは fix
- [ ] **HTTP レスポンスの JSON フィールド名 fix**
- [ ] **CHANGELOG の Migration ガイド整備** — 0.1.x → 0.1.0 の breaking 一覧
- [ ] **大規模 QA corpus** — `should_read.toml` の network coverage を増やす
  (現在 108 件)
- [ ] **branch protection 復元** — alpha 期間中の loose rule から stable 体制へ

#### timeline 見込み

「stable は 0.1.0」 + 「stable まで時間あり」 + **辞書完全再編成 + 特殊処理再設計** を含む大規模 refactor Plan X (2026-05-10 確定):

- **alpha.9 → alpha.10**: scoring-engine 投入 (Smart experimental) + dict format 拡張 + matcher (品詞除く) 実装 + 特殊処理再設計 + migration script 実装 + bracket forward compat
- **alpha.10 → alpha.11**: dict 完全再編成 PR series (機械変換 + entry purge + sub-dir 再構成)
- **alpha.11 → alpha.12〜N**: Smart bug fix + corpus calibration + dogfood (実時間 数ヶ月規模)
- **alpha.N → 0.1.0-rc1**: Smart default 切替、 corpus pass 100% 確認
- **0.1.0-rc1 → 0.1.0 stable**: 最終 sanity check 後、 full scoring-engine + 完全再編成済 dict で SemVer 開始
- **0.1.0 → 0.1.x patch**: dict 漸進拡充 / corpus 増強 / bug fix (additive only)
- **0.1.x → 0.2.0 stable**: intonation + 辞書側韻律対応投入 (bracket parse / `--mode=accent` / tts mode に accent 機能追加)
- **0.2.0 → 0.3.0+**: Strict 削除、 Lindera 信頼度再評価、 連濁 / 動詞活用 accent shift 等

stable cut までの実時間見積もり: **半年〜1 年規模**、 完成度優先 (期日 driven ではない)。

「alpha.9 を最終 alpha」 policy は **再撤回** (前回 intonation Postponed で再有効化したものを、 scoring-engine 0.1.0 入りに伴い再度撤回)。 stable cut は **lib (engine) 側の readiness 駆動**、 期日 driven でも dict data 充実駆動でもない:

**0.1.0 stable の position**: 「文脈依存ルビ振りが**確実に動く**段階」 で cut。 intonation 等の辞書側韻律対応は **0.2.0 stable target** ([intonation.md](./PROPOSALS/intonation.md) §0)。

**lib 側 必須要件**:
- Smart engine 実装完成 (bug 解消、 stable 動作)
- 既存 corpus regression (`should_read.toml` 現 108 件) が Smart engine で pass
- **文脈依存ルビ振りの動作 verification**: 全 matcher (`prev/next_eq` / `prev/next_pos` / `prev/next_char_type` / `_any` variants) が test pass + 代表的同形異音語 (上手 / 下る / 行った / 人気 / 一日 / 上下 等) を minimal corpus で pass
- migration script `tools/migrate_v2.py` 完成 (dict 側 maintainer が走らせる準備状態)
- public API freeze (`Furigana` builder / `Dict` / `RulesData` 等)
- `analyze()` API schema freeze
- HTTP server response JSON freeze
- 旧 format reject 実装、 matcher vocabulary 完全実装、 (a)(b)(c) penalty 数値 fix
- CHANGELOG / MIGRATION.md / doc 整備

**dict 側は cut 要件外** (0.1.x 漸進、 別 release cycle):
- `[[kanji]]` default reading の常用漢字制覇度 / 文脈 match data の量
- corpus regression 件数の増強 (現 108 件 pass で十分)

dict 側は lib 0.1.0 release と coordinated に migration commit + v0.1.0 tag を 1 回打つ、 以降は dict 独立 release cycle で漸進。

### 0.2.x 候補 (0.1.0 後)

#### intonation 関連の続編

- [ ] **`mode=voicevox-query`** — VOICEVOX `/synthesis` 直叩き用 AccentPhrase[] JSON 出力
  (pitch 値 / mora pause length 込み、 voicevox-aques 経由しなくて済む)
- [ ] **動的 accent shift rules** — 連濁 / 動詞活用 / 複合語 deaccenting / 助数詞 拡充
- [ ] **rules/accent/ の中身を地道に拡充** — 接頭辞 / 接尾辞 / 各 counter
- [ ] **NHK アクセント新辞典 出典の bulk PR** — 出典 license 確認後、 まとまった量の seed PR
- [ ] **engine adapter の community 受付** — openjtalk / ssml / ymm4 等は community PR 待ち、
  必要なら engine config 外部 TOML 化アーキテクチャを検討
- [ ] **user_dict CSV 化 検討** — 0.1.0 stable 1〜3 ヶ月運用後、 同形異音語 misclassification や
  複合語 boundary ずれが頻発するなら検討

### Phase 7 候補 (0.1.0 後)

- [ ] **作品単位辞書の継続拡充** — `core/works/` 構造に他作品を PR ベースで追加、
  サブポリシー (公式読みのみ採録 + 出典 comment 必須) を満たすもの
- [ ] **`lindera-neologd` opt-in feature flag** ([Issue #9](https://github.com/RyuuNeko1107/ja-furigana/issues/9))
  - 新語 / 商標 / アニメ作品名等が default で読めるようになる
  - 一方で binary 肥大化 (~50 MB → 数百 MB)、 NEologd は upstream 凍結中、
    過剰な複合語化の懸念
  - feature flag で choice にする案
- [ ] **辞書ピンの依存表記** — `Cargo.toml` 経由で辞書 version を declare できるように?
  - `cargo install ja-furigana-cli --features dict-pinned` のような切り口
- [ ] **postprocess ルールの拡充** — 土台 (mode 別 regex) はあるが具体ルールは少数。
  汎用的に使える rule を蓄積する
- [ ] **検証バッチからの corpus promote** — `tools/verify_batch.txt` で見つけた
  empirical な誤読修正を `ja-furigana-dict/tests/corpus/should_read.toml` に
  promote して回帰検証に組み込む

## 長期 vision (1.0+)

### 形態素解析依存の段階的撤廃

現在は `Lindera + IPADIC` で tokenize し、 `ja-furigana-dict` で override する 4 層構造。
dict 規模が 50k → 200k → 500k と育つにつれ、 Lindera が貢献する文脈が逓減する。

```
将来 vision (0.3.x or 1.0+):
入力 → ja-furigana-dict (longest-match + 活用 rule + 助詞 boundary) → 出力
        ↑ pure Rust、 deps 最小、 軽量、 license clean
```

便益:
- pure Rust deps 最小化
- 配布物軽量化 (Lindera + IPADIC 同梱で binary 数 MB)
- license obligation 減 (Lindera は MIT、 IPADIC は BSD だが、 撤廃で完全 control)

必要条件:
- dict が 200k+ entries (現在 50k)
- 動詞活用 rule layer
- 助詞 boundary detector
- 形態素解析無しで品質保てる corpus regression

0.1.0 stable で固める accent annotation の流儀 (TOML bracket、 user_dict CSV 不採用)
は、 この方向と整合する: accent は dict TOML、 形態素解析の中ではない。

## 廃止された候補

過去に検討したが、 別アプローチで代替したもの:

- ❌ **WebAssembly ビルド** — `.wasm` が Lindera + IPADIC 込みで 57 MB と重く、
  ブラウザから直接ロードするには不向きだった。 Web からは `furigana serve` (HTTP API)
  で十分という判断で削除 (alpha.4)
- ❌ **本体バイナリへの辞書 embed** — バイナリ肥大化 / 利用者ごとの辞書差し替え不能 /
  PR ループの遅さで却下。 `furigana-dict` 別 repo + `furigana dict pull` の構成に

## ロードマップ更新ポリシー

- 完了したものは [CHANGELOG.md](../CHANGELOG.md) `[Unreleased]` に移し、 本書からは
  サマリ 1 行に圧縮
- 大きい設計判断は本書ではなく [ARCHITECTURE.md](./ARCHITECTURE.md) に書く
