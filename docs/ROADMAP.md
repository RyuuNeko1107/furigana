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

- [ ] **公開 API のシグネチャ最終確認** — rename したくないものは fix
- [ ] **HTTP レスポンスの JSON フィールド名 fix**
- [ ] **CHANGELOG の Migration ガイド整備** — 0.1.x → 0.1.0 の breaking 一覧
- [ ] **大規模 QA corpus** — `should_read.toml` の network coverage を増やす
  (現在 108 件)
- [ ] **branch protection 復元** — alpha 期間中の loose rule から stable 体制へ

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
