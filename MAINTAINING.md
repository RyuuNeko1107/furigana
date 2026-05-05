# Maintaining ja-furigana

メンテナー (主に未来の自分) 向けの運用ガイド。release / publish / yank の手順、
失敗時の対応、token 管理などを記録する。

利用者向けの説明は [README.md](./README.md) を見てください。

---

## Release を打つ (binary 配布 + crates.io publish)

### 前提
- master が緑 (CI / lint / license audit すべて pass)
- `CHANGELOG.md` の `[Unreleased]` セクションを新バージョン名にリネームし、
  日付と diff URL を追記してから commit
- 動作確認: `cargo run -p ja-furigana-cli -- lookup '灰桜の散る道' --mode ruby` が正常

### 手順

```sh
# 1. workspace の version を bump
#    ルート `Cargo.toml` の [workspace.package].version
#    例: 0.1.0-alpha.2 → 0.1.0-alpha.3
#    `crates/furigana-cli/Cargo.toml` の依存表記
#    (`furigana = { package = "ja-furigana", path = "../furigana", version = "=0.1.0-alpha.X" }`)
#    も合わせて更新

# 2. CHANGELOG.md を整理して commit
git add CHANGELOG.md Cargo.toml crates/furigana/Cargo.toml crates/furigana-cli/Cargo.toml
git commit -m "chore(release): bump to 0.1.0-alpha.3"
git push origin master

# 3. tag を打って push
git tag -a v0.1.0-alpha.3 -m "v0.1.0-alpha.3 - <要約>"
git push origin v0.1.0-alpha.3

# 4. GitHub Actions の release workflow が走る (5 platform binary + Docker)
gh run watch --repo RyuuNeko1107/ja-furigana --workflow=release.yml

# 5. 確認
gh release view v0.1.0-alpha.3 --repo RyuuNeko1107/ja-furigana

# 6. crates.io にも publish (順序重要: lib → cli)
cargo publish -p ja-furigana
# ↑ index 反映待ちで数十秒〜数分。完了を待ってから次。
cargo publish -p ja-furigana-cli
```

### よくある失敗

#### `release_not_found` (binary upload で失敗)
`taiki-e/upload-rust-binary-action` は **release を作らない** (upload 専用)。
`release.yml` の `create-release` job が先頭にあるはずだが、もし古い workflow で
release が無ければ `gh release create` で空 release を先に作る:

```sh
gh release create v0.1.0-alpha.3 --repo RyuuNeko1107/ja-furigana \
  --target master --title v0.1.0-alpha.3 --generate-notes
gh run rerun <run-id> --repo RyuuNeko1107/ja-furigana --failed
```

#### tag を delete + 再 push したら workflow が trigger されない
GitHub の挙動で、同名 tag の delete + 再 push は push event として発火しない
ことがある。手動で trigger:

```sh
gh workflow run release.yml --repo RyuuNeko1107/ja-furigana -f tag=v0.1.0-alpha.3
```

#### `cargo fmt --check` で fail
ローカルで Windows ビルドだけ確認した時に起きやすい (CI は Linux で
fmt --check が走る)。

```sh
cargo fmt --all
git add -A && git commit -m "fix: cargo fmt"
```

#### Linux / macOS だけビルド失敗
Windows 上の `#[cfg(unix)]` でガードされたコードが Linux で動くか確認できない。
`crates/furigana-cli/src/commands/serve/mod.rs` の SIGHUP loop あたりが要注意。
ローカルで `cargo check --target x86_64-unknown-linux-gnu` (cross 必要) は
仕掛けが重いので、CI に任せて push → fail → fix のサイクルで進めて良い。

---

## crates.io の token 管理

### scope の使い分け

| scope | 何ができる | いつ要る |
|---|---|---|
| `publish-new` | 新規 crate の publish | 初回 publish 時 / 新 crate name 切替 |
| `publish-update` | 既存 crate の新バージョン publish | 通常の bump release 時 |
| `yank` | publish 済みバージョンを yank | 誤 publish の取り消し時 |
| `change-owners` | crate のオーナー変更 | 共同メンテナー追加時のみ |

普段使いは **`publish-new` + `publish-update` + `yank`** の 3 つを 1 つの
token に持たせると毎回切替不要。

### token 紛失時

`cargo login` した token は `~/.cargo/credentials.toml` に平文保存される。
公開環境 (CI 等) に流出した疑いがあれば即:
1. https://crates.io/me/ で該当 token を Revoke
2. 新 token を発行
3. `cargo login` しなおす

---

## yank する

```sh
cargo yank --version 0.1.0-alpha.X <crate-name>
# 例
cargo yank --version 0.1.0-alpha.1 furigana-cli
```

- yank しても crate name 自体は永久に自分が保持 (他人は取れない)。
- yank 後も既存 `Cargo.lock` 経由の DL は可能 (新規 `cargo add` だけブロック)。
- yank の取り消しは `cargo yank --undo` で可能。

---

## furigana-dict の release を CLI に反映する

`furigana dict pull` は GitHub Releases API で `ja-furigana-dict` の latest tag を
解決する。新しい辞書 release が出たら:

1. `ja-furigana-dict` 側で tag を打つ → release.yml が走って tarball + sha256 公開
2. CLI 側 (`ja-furigana-cli`) のコード変更は **不要** (latest を runtime で解決)
3. ピン留めしている利用者向けには CLI の README で `--version v0.1.X` 例を更新

`dict_pull.rs` の `REPO` 定数を変える必要があるのは「組織名 / repo 名」が変わった時だけ
(過去に `furigana-dict` → `ja-furigana-dict` rename で必要になった)。

---

## CI / Pages / Dependabot

### CI (`ci.yml`)
- `test` (3 OS) / `lint` (fmt + clippy) / `license` (cargo-about で copyleft 検知)
- 失敗を放置せず必ず fix。fmt 違反は再 commit、clippy 違反は対応。
- license job が `about.toml` 未許可の license を検知したら、依存追加時に
  `accepted` リストに追加するか、依存を別物に切替える判断。

### Dependabot (`.github/dependabot.yml`)
- 週次 (月曜 09:00 JST) で cargo + github-actions の更新 PR が来る。
- group 化されているので関連 crate (tokio-stack / lindera / serde-stack) が
  1 PR にまとまる。CI 緑なら merge してよい。
- breaking change を含む major bump は手動レビュー必須。

### Pages (なし、削除済み)
WASM crate と一緒に削除した。再導入する場合は過去 commit (`88ee9bc` 以前) を参照。

---

## バグ / Security 報告

- 一般 bug: GitHub Issues。`bug_report.yml` テンプレが立つ。
- security: 公開 issue ではなく email (Cargo.toml の `authors` に書いてあるアドレス) に
  まずプライベートに報告してもらう。CVE が必要なら GitHub の private vulnerability
  reporting (Settings → Security → Private vulnerability reporting) を有効化。

---

## ロードマップ

[`docs/ROADMAP.md`](./docs/ROADMAP.md) を最新に保つ。完了したものは
[`CHANGELOG.md`](./CHANGELOG.md) `[Unreleased]` に移し、ROADMAP.md からは消す。
README には status の概要だけ書き、詳細は ROADMAP.md に集約する方針。
