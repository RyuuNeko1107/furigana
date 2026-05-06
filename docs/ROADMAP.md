# ロードマップ

ja-furigana の中長期計画。**完了履歴は [CHANGELOG.md](../CHANGELOG.md)** を参照。本書は「これから何をやるか」志向で書く。

> 戻る: [README](../README.md)

## ステータス概観

**v0.1.x (alpha)**: Phase 1/2 機能はすべて動作。`0.1.x` の間は以下が予告なく変更されうる:

- 公開 Rust API (`Furigana` / `FuriganaBuilder` のメソッドシグネチャ)
- `furigana-dict` の TOML スキーマ (新フィールド追加、廃止)
- CLI 引数の名前 / デフォルト値
- HTTP レスポンスの JSON フィールド名 / 構造

安定版 (0.1.0 正式) 以降は SemVer で互換を守る。Rust toolchain は **1.88+** が必要。

## 完了済み

詳細は [CHANGELOG.md](../CHANGELOG.md) で。サマリのみ:

### Phase 1 (~2026-05-04)
- workspace + lib + CLI + データ駆動ルール (全 TOML)
- HTTP server (Axum、本番 ryuuneko.com API 互換)
- 辞書管理コマンド
- GitHub Release ワークフロー (5 platform binary + Docker image)
- 数値テキスト全体オーケストレーション (NumberChunker)
- [`furigana-dict`](https://github.com/RyuuNeko1107/ja-furigana-dict) リポジトリ開設

### Phase 2 (~2026-05-05)
- 本番 ryuuneko.com から `furigana-dict` への辞書 seed 投入 (unihan 43,749 / jukugo 605 / compat 436)
- `furigana dict pull` (GitHub Releases + SHA-256 検証 + 展開)
- 辞書のホットリロード (`SIGHUP` / `POST /admin/reload`)
- portable 配置 (`furigana.exe` 横に `data/` 1 階層集約)
- 対話 REPL (`furigana repl` / 引数なし起動 / Tab 補完 / 履歴 / `:` optional)
- SI 単位の case-insensitive lookup
- 四字熟語の分離 (`core/jukugo/four_char.toml`)
- crates.io 公開 (`ja-furigana` lib + `ja-furigana-cli` bin)
- ローマ字出力モード (ヘボン式 / 訓令式)
- Lindera analyzer の lazy init (`Furigana::minimal()` で 5.97 ms → 27.3 µs)

### Phase 3 (~2026-05-06、0.1.0-alpha.3 で完了)
- **本番 ryuuneko.com の公開フリガナ API パイプライン互換** に揃えた読み解決優先順位
  (`context rule → jukugo → Lindera → unihan` の 5 段階、`resolve_reading`)
- **`Dict` を `jukugo` (≥2 文字) / `unihan` (1 文字) に内部分離** + 専用 lookup API
- **NumberChunker** の改修:
  - 漢数字対応 (一〜二十一を Arabic に変換、`kansuji_to_arabic`)
  - 「6月一日」のような Arabic+漢数字混在日付が日付 chunk として認識
  - counter「N日」単独 = 期間扱い、日付内のみ days.toml の特殊読み (1=ツイタチ等) を採用
  - scale + 漢字 1 文字 unit の連結 (「1万円」「3億ドル」等を 1 chunk で)
- **`postprocess.toml`** (本番 Step 7 互換) — mode 別 regex 置換ルールの土台
- **辞書大規模拡充** (jukugo 605 → 1,163、+90%) — animals / foods / specialized /
  body_parts / weather / colors / arts / abstracts の 8 ファイル新設
- **検証ループ駆動の品質改善** — `tools/check_samples.txt` (75 件) を回帰検証で
  75/75 (100%) 達成
- CI: macOS test を週次 schedule に移動、cargo-audit + corpus regression job 追加

## 進行中 / 候補

### Phase 4 候補

- [ ] **0.1.0 正式版へ昇格** — alpha → 安定版。リリース前に確認するもの:
  - 公開 Rust API のシグネチャ最終確認 (rename したくないものは fix)
  - HTTP レスポンスの JSON フィールド名 fix
  - CHANGELOG の Migration ガイド
- [ ] **人名・固有名詞の更なる拡充** — 0.1.2 で seed 投入済 (personal_names 71 /
  proper_nouns 67) だが、現代の私人 / 私企業 / 作品名は手付かず。PR ベースで順次。
  - 詳細は [`ja-furigana-dict/STATS.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/STATS.md)
- [ ] **`lindera-neologd` opt-in feature flag** — [Issue #9](https://github.com/RyuuNeko1107/ja-furigana/issues/9)
  - 新語 / 商標 / アニメ作品名等が default で読めるようになる
  - 一方で binary 肥大化 (~50 MB → 数百 MB)、NEologd は upstream 凍結中、過剰な複合語化の懸念
  - feature flag で choice にする案
- [ ] **辞書ピンの依存表記** — `Cargo.toml` 経由で辞書 version を declare できるように?
  - `cargo install ja-furigana-cli --features dict-v0.1.2` のような切り口
- [ ] **postprocess ルールの拡充** — 0.1.0-alpha.3 で土台はできたが、現状の rule は
  「ジュウパー → ジュッパー」1 件のみ。汎用的に使える rule を蓄積する
- [ ] **検証ループ corpus への promote** — `tools/check_samples.txt` で 0.1.0-alpha.3〜.5
  にかけて 100 件の誤読を直したが、回帰防止のためにそれらを `(input, mode, expected)`
  で `ja-furigana-dict/tests/corpus/should_read.toml` に蓄積する作業がまだ未完。
  約 40 件の heteronym + 単漢字音読み正規化系を優先してプロモートする想定。
  CI 側の corpus job (`tools/run_corpus.py`) は既に組み込み済なので、入れ次第回帰検知が効く

## 廃止された候補

過去に検討したが、別アプローチで代替したもの:

- ❌ **WebAssembly ビルド** — 一度実装したが `.wasm` が Lindera + IPADIC 込みで 57 MB と重く、ブラウザから直接ロードするには不向きだった。Web からは `furigana serve` (HTTP API) で十分という判断で削除。CHANGELOG `[Unreleased]` の Removed セクション参照
- ❌ **本体バイナリへの辞書 embed** — バイナリ肥大化 / 利用者ごとの辞書差し替え不能 / PR ループの遅さで却下。`furigana-dict` 別 repo + `furigana dict pull` の構成に

## ロードマップ更新ポリシー

- 完了したものは [CHANGELOG.md](../CHANGELOG.md) `[Unreleased]` に移し、本書からは消す
- Phase 4 以降を書き始める時は本書を分割せず追記する (規模次第で再考)
- 大きい設計判断は本書ではなく [ARCHITECTURE.md](./ARCHITECTURE.md) に書く
