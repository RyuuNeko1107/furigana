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

## 進行中 / 候補

### Phase 3 (進行中)

- [ ] **0.1.0 正式版へ昇格** — alpha → 安定版。リリース前に確認するもの:
  - 公開 Rust API のシグネチャ最終確認 (rename したくないものは fix)
  - HTTP レスポンスの JSON フィールド名 fix
  - CHANGELOG の Migration ガイド
- [ ] **人名・固有名詞の手動振り分け** — 機械分類が困難なため、PR で順次拡充
  - `core/jukugo/personal_names.toml` (現状 0 件)
  - `core/jukugo/proper_nouns.toml` (現状 0 件)
  - 詳細は [`ja-furigana-dict/STATS.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/STATS.md)

### Phase 3 候補 (検討中)

- [ ] **`lindera-neologd` opt-in feature flag** — [Issue #9](https://github.com/RyuuNeko1107/ja-furigana/issues/9)
  - 新語 / 商標 / アニメ作品名等が default で読めるようになる
  - 一方で binary 肥大化 (~50 MB → 数百 MB)、NEologd は upstream 凍結中、過剰な複合語化の懸念
  - feature flag で choice にする案
- [ ] **回帰テストの自動化** — `ja-furigana-dict/tests/corpus/should_read.toml` の各 case を CI で回す
  - 現状は人が眺める材料 (`should_read.toml` / `should_not_read_yet.toml` / `out_of_scope.toml`)
- [ ] **辞書ピンの依存表記** — `Cargo.toml` 経由で辞書 version を declare できるように?
  - `cargo install ja-furigana-cli --features dict-v0.1.1` のような切り口

## 廃止された候補

過去に検討したが、別アプローチで代替したもの:

- ❌ **WebAssembly ビルド** — 一度実装したが `.wasm` が Lindera + IPADIC 込みで 57 MB と重く、ブラウザから直接ロードするには不向きだった。Web からは `furigana serve` (HTTP API) で十分という判断で削除。CHANGELOG `[Unreleased]` の Removed セクション参照
- ❌ **本体バイナリへの辞書 embed** — バイナリ肥大化 / 利用者ごとの辞書差し替え不能 / PR ループの遅さで却下。`furigana-dict` 別 repo + `furigana dict pull` の構成に

## ロードマップ更新ポリシー

- 完了したものは [CHANGELOG.md](../CHANGELOG.md) `[Unreleased]` に移し、本書からは消す
- Phase 4 以降を書き始める時は本書を分割せず追記する (規模次第で再考)
- 大きい設計判断は本書ではなく [ARCHITECTURE.md](./ARCHITECTURE.md) に書く
