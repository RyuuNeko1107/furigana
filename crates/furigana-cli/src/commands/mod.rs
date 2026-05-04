//! サブコマンド実装

pub mod dict;
pub mod lookup;
pub mod serve;

use crate::paths::Paths;
use anyhow::Result;
use furigana::Furigana;

/// CLI サブコマンドが共通で使う Furigana インスタンスを構築する
///
/// `<data_dir>/dict/{core,user}/` と `overrides.tsv` を自動的にマウント。
/// 何もファイルが無い場合は埋め込みルール + 空辞書で起動。
pub fn build_furigana(paths: &Paths) -> Result<Furigana> {
    let mut b = Furigana::builder();

    let core = paths.dict_core_dir();
    if core.exists() {
        b = b.core_dict_dir(&core);
    }
    let user = paths.dict_user_dir();
    if user.exists() {
        b = b.user_dict_dir(&user);
    }
    let overrides = paths.overrides_file();
    if overrides.exists() {
        b = b.overrides_file(&overrides);
    }

    Ok(b.build()?)
}
