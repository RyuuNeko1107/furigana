//! `furigana serve` サブコマンド (Task #11 で実装)

use crate::config::Config;
use crate::paths::Paths;
use anyhow::{bail, Result};
use clap::Args as ClapArgs;

/// `furigana serve` のオプション
#[derive(ClapArgs, Debug)]
pub struct Args {
    /// bind address (config.toml の値を上書き)
    #[arg(long)]
    pub bind: Option<String>,

    /// 認証トークンを 1 個だけ env で渡す簡易モード (config 不要)
    #[arg(long, env = "FURIGANA_TOKEN")]
    pub token: Option<String>,
}

/// 実行 — まだ未実装
pub fn run(_args: Args, _paths: &Paths, _cfg: &Config) -> Result<()> {
    bail!("`furigana serve` は未実装です (Task #11)");
}
