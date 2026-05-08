## 変更内容

<!-- 例: NumberChunker に range (3〜5本) 検出を追加 -->

## 変更種別

- [ ] feat: 新機能
- [ ] fix: バグ修正
- [ ] refactor: 振る舞い変更なしのリファクタ
- [ ] docs: ドキュメント
- [ ] ci: ビルド / CI
- [ ] chore: その他

## チェックリスト

- [ ] `cargo fmt --all -- --check` 通過
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` クリーン
- [ ] `cargo test --workspace` 通過
- [ ] 新規公開 API には rustdoc コメント
- [ ] (振る舞い変更がある場合) 1 件以上のテスト追加
- [ ] (feat / fix の場合) この PR がカバーした input/expected を **`ja-furigana-dict/tests/corpus/should_read.toml`** または該当 file の `*.test.toml` に **1 件以上追加** (回帰防止)
- [ ] (schema 変更を伴う場合) `furigana-dict` 側にも対応 PR を出した

## 関連 Issue / PR

<!-- 例: Closes #42 / furigana-dict#5 と連動 -->
