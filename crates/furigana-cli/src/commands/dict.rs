//! `furigana dict ...` サブコマンド一式 (Task #11 で実装)
//!
//! - `add <surface> <reading>` : user dict に 1 件追加
//! - `list [--limit N]`         : 現在の辞書サイズ + サンプル
//! - `remove <surface>`         : user dict から削除
//! - `import <path>`            : TSV ファイルを user dict にマージ
//! - `pull [--version v...]`    : GitHub Release から core 辞書を DL

use crate::config::Config;
use crate::paths::Paths;
use anyhow::{bail, Result};
use clap::{Args as ClapArgs, Subcommand};

/// `furigana dict ...` 親オプション
#[derive(ClapArgs, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub action: Action,
}

/// dict サブコマンド
#[derive(Subcommand, Debug)]
pub enum Action {
    /// user 辞書に 1 件追加 (重複は上書き)
    Add {
        /// 表層形 (漢字を含む語)
        surface: String,
        /// カタカナ読み
        reading: String,
    },

    /// 現在の辞書状態をサマリ表示
    List {
        /// 表示するエントリ数の上限
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },

    /// user 辞書から削除
    Remove {
        /// 削除する表層形
        surface: String,
    },

    /// TSV ファイルを user 辞書にマージ
    Import {
        /// インポート元 TSV
        path: std::path::PathBuf,
    },

    /// GitHub Release から core 辞書を取得
    Pull {
        /// ピン留めバージョン (例: v0.1.0)。未指定で最新。
        #[arg(long)]
        version: Option<String>,
    },
}

/// 実行 — まだ未実装
pub fn run(_args: Args, _paths: &Paths, _cfg: &Config) -> Result<()> {
    bail!("`furigana dict ...` は未実装です (Task #11)");
}
