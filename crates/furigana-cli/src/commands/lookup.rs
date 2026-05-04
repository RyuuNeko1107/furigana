//! `furigana lookup` サブコマンド
//!
//! 1 回だけ変換してそれを stdout に出して終了する CLI。
//! サーバー起動なし、即時 1 ショット用途。
//! 公開 API の `mode` パラメータと同じ 4 種に対応。

use crate::config::Config;
use crate::paths::Paths;
use anyhow::{bail, Result};
use clap::Args as ClapArgs;
use furigana::TtsOptions;

/// `furigana lookup` のオプション
#[derive(ClapArgs, Debug)]
pub struct Args {
    /// 変換対象テキスト
    text: String,

    /// 変換モード: `tts` (default) | `hiragana` | `ruby` | `kanji`
    #[arg(short, long, default_value = "tts")]
    mode: String,

    /// TTS: 「、」後に挿入する文字列
    #[arg(long, default_value = " ")]
    short_pause: String,

    /// TTS: 「。!?」後に挿入する文字列
    #[arg(long, default_value = "   ")]
    long_pause: String,

    /// TTS: `。` を残さず削除する
    #[arg(long)]
    drop_period: bool,
}

/// 実行
pub fn run(args: Args, paths: &Paths, _cfg: &Config) -> Result<()> {
    let f = super::build_furigana(paths)?;

    let result = match args.mode.as_str() {
        "kanji" => args.text.clone(),
        "ruby" => f.to_ruby(&args.text),
        "hiragana" | "hira" => f.to_hiragana(&args.text),
        "tts" => {
            let opts = TtsOptions {
                short_pause: args.short_pause,
                long_pause: args.long_pause,
                keep_period: !args.drop_period,
            };
            f.to_tts(&args.text, &opts)
        }
        other => bail!("未知の mode: {other} (使用可能: tts | hiragana | ruby | kanji)"),
    };

    println!("{result}");
    Ok(())
}
