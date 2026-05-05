//! `furigana repl` サブコマンド
//!
//! 対話モード。1 行入力すると現在の mode で変換して即時出力。
//! `:` プレフィクスでメタコマンド (`:help` で一覧)。
//!
//! 依存追加なしで `std::io::stdin()` のみ使用 (history / 矢印キーは未対応)。

use crate::config::Config;
use crate::paths::Paths;
use anyhow::Result;
use clap::Args as ClapArgs;
use furigana::{Furigana, TtsOptions};
use std::io::{self, BufRead, Write};
use std::time::Instant;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// 起動時の mode (default: `all`)
    #[arg(long, default_value = "all")]
    mode: String,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            mode: "all".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// ruby + hiragana を 2 行で同時表示 (試す用途のデフォルト)
    All,
    Ruby,
    Hiragana,
    Tts,
    Kanji,
}

impl Mode {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "all" => Self::All,
            "ruby" => Self::Ruby,
            "hiragana" | "hira" => Self::Hiragana,
            "tts" => Self::Tts,
            "kanji" => Self::Kanji,
            _ => return None,
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Ruby => "ruby",
            Self::Hiragana => "hiragana",
            Self::Tts => "tts",
            Self::Kanji => "kanji",
        }
    }
}

pub fn run(args: Args, paths: &Paths, _cfg: &Config) -> Result<()> {
    let Args { mode: initial_mode } = args;
    let mut f = super::build_furigana(paths)?;
    let mut mode = Mode::parse(&initial_mode).unwrap_or(Mode::All);
    let mut debug = false;

    eprintln!("furigana REPL");
    eprintln!("  dict_size: {}", f.dict_size());
    if f.dict_size() == 0 {
        eprintln!("  (辞書が空です。`:pull` で furigana-dict を取得するとフリガナ精度が上がります)");
    }
    eprintln!("  type :help for commands, Ctrl-D to quit");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        write!(stdout, "{}> ", mode.as_str())?;
        stdout.flush()?;

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                writeln!(stdout)?;
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix(':') {
            let mut parts = rest.splitn(2, char::is_whitespace);
            let cmd = parts.next().unwrap_or("");
            let arg = parts.next().unwrap_or("").trim();
            match cmd {
                "h" | "help" => print_help(&mut stdout)?,
                "q" | "quit" | "exit" => break,
                "size" => writeln!(stdout, "dict_size: {}", f.dict_size())?,
                "r" | "reload" => match super::build_furigana(paths) {
                    Ok(new) => {
                        f = new;
                        writeln!(stdout, "reloaded. dict_size: {}", f.dict_size())?;
                    }
                    Err(e) => writeln!(stdout, "reload failed: {e}")?,
                },
                "pull" => {
                    let version = if arg.is_empty() { None } else { Some(arg) };
                    match super::dict_pull::run(paths, version) {
                        Ok(()) => match super::build_furigana(paths) {
                            Ok(new) => {
                                f = new;
                                writeln!(
                                    stdout,
                                    "pull + reload 完了。dict_size: {}",
                                    f.dict_size()
                                )?;
                            }
                            Err(e) => writeln!(stdout, "reload failed: {e}")?,
                        },
                        Err(e) => writeln!(stdout, "pull failed: {e}")?,
                    }
                }
                "mode" => {
                    if arg.is_empty() {
                        writeln!(stdout, "current mode: {}", mode.as_str())?;
                        writeln!(
                            stdout,
                            "available: all | ruby | hiragana | tts | kanji"
                        )?;
                    } else if let Some(m) = Mode::parse(arg) {
                        mode = m;
                        writeln!(stdout, "mode -> {}", mode.as_str())?;
                    } else {
                        writeln!(stdout, "unknown mode: {arg}")?;
                    }
                }
                "debug" => {
                    debug = !debug;
                    writeln!(stdout, "debug: {}", if debug { "on" } else { "off" })?;
                }
                "tokens" => {
                    if arg.is_empty() {
                        writeln!(stdout, "usage: :tokens <text>")?;
                    } else {
                        dump_tokens(&mut stdout, &f, arg)?;
                    }
                }
                other => writeln!(stdout, "unknown command: :{other} (try :help)")?,
            }
            continue;
        }

        // 通常の入力 → 変換
        let t0 = Instant::now();
        let tokens = f.tokenize(line);
        let t_tok = t0.elapsed();

        let t1 = Instant::now();
        match mode {
            Mode::All => {
                writeln!(stdout, "  ruby:     {}", furigana::tokens_to_ruby(&tokens))?;
                writeln!(
                    stdout,
                    "  hiragana: {}",
                    furigana::tokens_to_hiragana(&tokens)
                )?;
            }
            Mode::Ruby => writeln!(stdout, "{}", furigana::tokens_to_ruby(&tokens))?,
            Mode::Hiragana => writeln!(stdout, "{}", furigana::tokens_to_hiragana(&tokens))?,
            Mode::Tts => {
                let opts = TtsOptions::default();
                let hira = furigana::tokens_to_hiragana(&tokens);
                writeln!(stdout, "{}", furigana::tts::normalize_for_tts(&hira, &opts))?;
            }
            Mode::Kanji => writeln!(stdout, "{line}")?,
        }
        let t_conv = t1.elapsed();

        if debug {
            writeln!(
                stdout,
                "  [debug] tokenize {:.2}ms / convert {:.2}ms / total {:.2}ms",
                t_tok.as_secs_f64() * 1000.0,
                t_conv.as_secs_f64() * 1000.0,
                (t_tok + t_conv).as_secs_f64() * 1000.0,
            )?;
        }
    }
    Ok(())
}

fn dump_tokens(w: &mut impl Write, f: &Furigana, text: &str) -> io::Result<()> {
    let tokens = f.tokenize(text);
    if tokens.is_empty() {
        writeln!(w, "  (no tokens)")?;
        return Ok(());
    }
    let surface_w = tokens
        .iter()
        .map(|t| t.surface.chars().count())
        .max()
        .unwrap_or(0)
        .max(7);
    writeln!(w, "  {:width$}  reading", "surface", width = surface_w)?;
    writeln!(w, "  {:-<width$}  -------", "", width = surface_w)?;
    for t in &tokens {
        writeln!(
            w,
            "  {:width$}  {}",
            t.surface,
            t.reading.as_deref().unwrap_or("(none)"),
            width = surface_w
        )?;
    }
    Ok(())
}

fn print_help(w: &mut impl Write) -> io::Result<()> {
    writeln!(w, "Commands:")?;
    writeln!(w, "  :help          このヘルプ")?;
    writeln!(w, "  :mode <m>      mode 切替 (all|ruby|hiragana|tts|kanji)")?;
    writeln!(w, "  :debug         timing 表示の on/off (toggle)")?;
    writeln!(w, "  :tokens <text> 内部 token 配列を dump (なぜこの読み？を調べる用)")?;
    writeln!(w, "  :pull [vX.Y.Z] furigana-dict を取得 + 自動 reload (初回のセットアップに)")?;
    writeln!(w, "  :reload        data_dir から辞書を再 build")?;
    writeln!(w, "  :size          dict_size を表示")?;
    writeln!(w, "  :quit          終了 (Ctrl-D も可)")?;
    writeln!(w)?;
    writeln!(w, "プレフィクス無しの入力は現在の mode で変換して表示します。")?;
    Ok(())
}
