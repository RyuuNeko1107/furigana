# Changelog

このプロジェクト (`ja-furigana` lib + `ja-furigana-cli` bin) の変更履歴。
[Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) 形式に概ね従い、
バージョニングは [Semantic Versioning](https://semver.org/lang/ja/) を採用。

## [Unreleased]

### Added
- `Furigana::merge_dict_toml(content)` — TOML 文字列を辞書に一括 merge する API。
  ファイルシステムベースの `core_dict_dir` が使えない環境向け。
- `Furigana::preload()` — Lindera 形態素解析器を eager に初期化する API
  (server 起動時の preload 用)。
- `examples/clients/{python,nodejs,curl}/` — `furigana serve` HTTP API を他言語から
  叩く最小サンプル (TTS パイプライン / Discord bot / shell パイプ用途)。
- `crates/furigana/benches/lookup.rs` — criterion ベンチ (init / mode 別 / tokenize)。

### Changed
- **`Furigana` の Lindera 初期化を lazy に**。`Furigana::minimal()` /
  `FuriganaBuilder::build()` の時点では Analyzer を init せず、最初の
  `tokenize` / `to_*` 呼び出し時に [`OnceLock`] で 1 度だけ init。
  `Furigana::minimal()` 単体の bench で **5.97 ms → 27.3 µs (-99.5%)**。
  CLI レベルでは `--version` / `--help` 等の Lindera 不要経路が
  ~80 ms → ~10 ms に高速化。`furigana serve` は preload を起動時に呼んで
  最初のリクエストレイテンシを保つ。

### Removed
- `crates/furigana-wasm/` (WebAssembly bindings) を削除。`.wasm` が Lindera + IPADIC
  込みで 57 MB と重く、Web からは `furigana serve` (HTTP API) で十分という判断。
  Pages workflow (`.github/workflows/pages.yml`) も合わせて削除。
  `lib::merge_dict_toml` API は WASM 用に追加したが、サーバ無し環境からの利用にも
  汎用的に役立つので lib 側に残してある。

## [0.1.0-alpha.2] - 2026-05-06

### Changed
- crate と GitHub repo の名前統一: `furigana` (取得済) は別 crate に取られていたため
  `ja-furigana` (lib) / `ja-furigana-cli` (bin) に rename。
- GitHub repo を `RyuuNeko1107/furigana` → `RyuuNeko1107/ja-furigana` /
  `RyuuNeko1107/furigana-dict` → `RyuuNeko1107/ja-furigana-dict` に rename。
  GitHub redirect が効くため旧 URL も互換。
- `crates/furigana-cli/src/commands/dict_pull.rs` の REPO 定数を
  `RyuuNeko1107/ja-furigana-dict` に更新 (alpha.1 は redirect 経由)。

### Removed
- 旧 `furigana-cli@0.1.0-alpha.1` を yank (rename 前の crate name、利用者ほぼゼロ前提)。

## [0.1.0-alpha.1] - 2026-05-05

初回 crates.io publish。Phase 2 機能ほぼ完成版。

### Added (Phase 2)
- **`furigana repl`**: 対話モード (rustyline、Tab 補完、↑↓ 履歴、`:` optional)。
  引数なしで起動すれば REPL に入る (Windows ダブルクリック対応)。
- **`furigana dict pull`**: GitHub Releases から `ja-furigana-dict` の tarball を fetch、
  SHA-256 検証、`<data_dir>/data/` 配下に flat 展開。
- **ホットリロード**: `POST /admin/reload` (`[auth].admin_tokens` 認証) と Unix 上の
  `SIGHUP` で `<data_dir>` から辞書を再 build。
- **portable 配置**: 既定では `<exe>/data/` に展開。フォルダごとコピーで持ち運べる。
- **SI 単位の case-insensitive lookup**: `1km` / `1KM` / `1Km` どれも「いちきろめーとる」。
  個別 entry で `ci = false` opt-out 可能。
- **依存ライセンスの自動収集**: `cargo about` で `NOTICE.md` を生成。CI で license
  drift を検知 (GPL/AGPL の混入を防止)。
- **GitHub Releases 自動配布**: `release.yml` で 5 platform の binary +
  `ghcr.io/ryuuneko1107/furigana` Docker image を tag push で配布。

### Changed
- 配布物 layout を `<data_dir>/{core,rules}/` の 2 階層から `<data_dir>/data/`
  1 階層に統合。`Dict::from_toml_str` を defensive に修正し、rules 系
  inline-table TOML を silent skip するように。

## [Pre-history (Phase 1)] - ~2026-05-04

- workspace 構成 (`furigana` lib + `furigana-cli` bin) と Lindera + IPADIC ベースの
  形態素解析パイプライン。
- `Furigana` / `FuriganaBuilder` 公開 API、`tokens_to_ruby` / `tokens_to_hiragana`、
  TTS 整形 (`TtsOptions` + `normalize_for_tts`)。
- `furigana lookup` / `furigana serve` (Axum HTTP、本番 API 互換) /
  `furigana dict {add,list,remove,import}` サブコマンド。
- 数値テキスト全体オーケストレーション (`NumberChunker` で時刻・日付・URL・スケール・
  助数詞・SI 単位を 1 パイプラインで処理)。
- データ駆動ルール: 全ルールを `ja-furigana-dict` 側 TOML で外部化。
- 本番 ryuuneko.com から seed 投入 (unihan 43,749 / jukugo 605 / compat 436)。

## [一覧]

[Unreleased]: https://github.com/RyuuNeko1107/ja-furigana/compare/v0.1.0-alpha.2...HEAD
[0.1.0-alpha.2]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.2
[0.1.0-alpha.1]: https://github.com/RyuuNeko1107/ja-furigana/releases/tag/v0.1.0-alpha.1
