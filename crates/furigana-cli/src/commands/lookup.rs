//! `furigana lookup` サブコマンド
//!
//! 1 回だけ変換してそれを stdout に出して終了する CLI。
//! サーバー起動なし、即時 1 ショット用途。

use crate::config::Config;
use crate::paths::Paths;
use anyhow::{bail, Result};
use clap::Args as ClapArgs;

/// `furigana lookup` のオプション
#[derive(ClapArgs, Debug)]
pub struct Args {
    /// 変換対象テキスト
    text: String,

    /// 出力形式: `ruby` (default) | `hiragana`
    #[arg(short, long, default_value = "ruby")]
    format: String,
}

/// 実行
pub fn run(args: Args, paths: &Paths, _cfg: &Config) -> Result<()> {
    let f = super::build_furigana(paths)?;

    match args.format.as_str() {
        "ruby" => {
            println!("{}", f.to_ruby(&args.text));
        }
        "hiragana" | "hira" => {
            println!("{}", f.to_hiragana(&args.text));
        }
        other => bail!("未知の format: {other} (使用可能: ruby / hiragana)"),
    }

    Ok(())
}
