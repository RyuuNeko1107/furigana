# データ配置

辞書 / ルール TOML の置き場所 / 探索順 / 優先順位の解説。

> 戻る: [README](../README.md) / 関連: [RULES.md](./RULES.md) (各ファイルの中身) / [CONFIG.md](./CONFIG.md)

## 既定の場所 (portable 配置)

default では **実行ファイルと同じディレクトリ**:

```
<furigana.exe と同じフォルダ>/
├── furigana.exe                   # 本体 (Windows なら .exe / 他は単に furigana)
├── config.toml                    # 設定 (任意)
├── repl_history                   # REPL の入力履歴 (自動生成)
└── data/                          # `furigana dict pull` で展開、ユーザー追加もここに集約
    ├── unihan.toml                # 単漢字フォールバック (43k+ 字)
    ├── compat.toml                # 異体字マップ (髙→高 等)
    ├── jukugo/*.toml              # 熟語 / 固有名詞 / 地名 / 人名 (5 ファイル)
    ├── days.toml                  # 1〜31 日特殊読み
    ├── scales.toml                # 万 / 億 / 兆 / 京 / 垓 ...
    ├── units.toml                 # SI 単位
    ├── symbols.toml               # 記号読み
    ├── latin.toml                 # ラテン文字読み (A→エー …)
    ├── numeric_phrases.toml       # 慣用語句 (二十歳→ハタチ 等)
    ├── counters/*.toml            # 助数詞ルール (7 ファイル)
    ├── context/*.toml             # 文脈ルール (3 ファイル)
    ├── user/                      # ユーザー追加 (`furigana dict add` で自動生成)
    │   └── cli-added.toml         #   `furigana dict add` 経由のエントリ
    └── overrides.toml              # 強制上書き用 (最優先、任意)
```

zip / tar.gz を解凍したフォルダごとコピーすれば持ち運べる **portable 配置**。

> ⚠️ 旧バージョン (alpha.1 以前) には `data/core/` と `data/rules/` の 2 階層分けがあったが、alpha.2 で **`data/` 1 階層に統合** された。lib loader が「`[entries]` 持つ TOML だけ拾う dict scan」「特定ファイル名だけ拾う rules scan」を排他的に実行するため、同じ `data/` を両方に渡しても干渉しない。

## カスタム場所 + 環境変数

CLI flag および環境変数で `<data_dir>` を変えられる:

```sh
# CLI flag
furigana lookup '灰桜' --data-dir /var/lib/furigana

# 環境変数 (優先度: CLI flag > env)
export FURIGANA_DATA_DIR=/var/lib/furigana
furigana lookup '灰桜'
```

`current_exe()` の解決に失敗した稀なケースのみ XDG fallback:
- Linux: `~/.local/share/furigana/`
- Windows: `%LOCALAPPDATA%\furigana\furigana\`
- macOS: `~/Library/Application Support/com.furigana.furigana/`

## 優先順位 (高→低)

辞書 lookup 時にどの源を優先するか:

1. **`data/overrides.toml`** — `FuriganaBuilder::overrides_file()` で mount。最強の上書き
2. **`data/user/*.toml`** — `FuriganaBuilder::user_dict_dir()`。`furigana dict add` の保存先
3. **`data/*.toml` + `data/jukugo/*.toml`** — `FuriganaBuilder::core_dict_dir()`。`furigana dict pull` 配布版
4. **文脈ルール** (`data/context/*.toml`) — `FuriganaBuilder::rules_dir()` で mount。前後トークンを見て読みを決定
5. **Lindera (IPADIC)** — 形態素解析の素朴な読み。details[7] のカタカナ
6. **読みなし** (`None`) — どこにも hit しなければ surface のまま出力

`furigana dict add 灰桜 ハイザクラ` で `data/user/cli-added.toml` に追加 → 次回起動時 (or `:reload`) で反映。

## `furigana dict pull` の動き

GitHub Releases から `ja-furigana-dict` の tarball を取得 + SHA-256 検証 + 展開する。

```sh
furigana dict pull                       # 最新 release
furigana dict pull --version v0.1.1      # version pin
```

詳細フロー:

1. `--version` 指定 or GitHub API `/releases/latest` で tag 解決
2. `furigana-dict-{tag}.tar.gz` をダウンロード
3. `furigana-dict-{tag}.tar.gz.sha256` (sidecar) を取得して **SHA-256 検証** (sidecar 無い古い release は warn skip)
4. `<data_dir>/data/` 配下の旧配布ファイルを削除 (**`user/` と `overrides.toml` は保持**)
5. tarball 展開 + path rebase: archive 内の `core/X` / `rules/X` を `data/X` に flat 化

archive 側 (`ja-furigana-dict` repo) は `core/` `rules/` の 2 階層で PR レビュー上の分類のために維持されている。配布物だけ flat に変換される (展開後は `data/` 1 階層)。

```
archive 内 (PR レビュー用)        →   利用者の <data_dir>/data/ (flat)
core/jukugo/general.toml         →   data/jukugo/general.toml
core/unihan.toml                 →   data/unihan.toml
rules/days.toml                  →   data/days.toml
rules/counters/objects.toml      →   data/counters/objects.toml
rules/context/numbers.toml       →   data/context/numbers.toml
```

path traversal 防御として、archive entry の展開先が `<data_dir>/data/` 配下に収まることを canonicalize で確認している。

## REPL からの操作

```
all> :pull              # 同名コマンド (`furigana dict pull` 相当)
all> :pull v0.1.1       # version pin
all> :reload            # data_dir から in-memory 辞書を再 build
all> :size              # dict_size 表示
```

詳細は `furigana repl` 起動後 `:help` で。

## 関連リンク

- 各ファイルの中身 / フォーマット: [RULES.md](./RULES.md)
- 設定ファイル全項目: [CONFIG.md](./CONFIG.md)
- 辞書 PR の出し方: [`ja-furigana-dict/CONTRIBUTING.md`](https://github.com/RyuuNeko1107/ja-furigana-dict/blob/master/CONTRIBUTING.md)
