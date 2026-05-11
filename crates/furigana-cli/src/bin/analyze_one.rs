//! Single-input analyze dump tool (★dev only)。
//!
//! 任意のテキストに対し、 dict + rules を mount した状態で AnalyzeResult を
//! JSON pretty 出力する。 corpus_check は corpus TOML 経由なので、 1 入力を
//! ad-hoc に inspect したい時の dev tool。
//!
//! ```bash
//! cargo run --release --bin furigana-analyze-one -- \
//!     --rules-dir <path> --core-dict-dir <path> "美しい花が咲く"
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use furigana::Furigana;
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Args {
    /// 解析対象テキスト
    text: String,
    #[arg(long)]
    rules_dir: Option<PathBuf>,
    #[arg(long)]
    core_dict_dir: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut b = Furigana::builder();
    if let Some(rules) = &args.rules_dir {
        b = b.rules_dir(rules);
    }
    for d in &args.core_dict_dir {
        b = b.core_dict_dir(d);
    }
    let f = b.build().context("build Furigana")?;
    let result = f.analyze(&args.text);
    let json = serde_json::to_string_pretty(&result).context("serialize")?;
    println!("{json}");
    Ok(())
}
