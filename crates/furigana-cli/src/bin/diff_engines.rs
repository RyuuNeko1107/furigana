//! Smart vs Strict engine diff tool (alpha.10〜0.1.0-rc1 dogfood / CI 用)。
//!
//! 詳細仕様: `docs/PROPOSALS/scoring-engine.md` ★6
//!
//! ## 用途
//!
//! corpus file (例: `furigana-dict/tests/corpus/should_read.toml`) を入力に、
//! 各 case を **Strict / Smart 両 engine** で実行、 出力 diff を表示する。
//! exit code: diff あれば非 0 (CI で監視可能)。
//!
//! 0.1.0-rc1 で Smart default 切替前の最終 sanity check として使う。
//! alpha.10 段階では Smart engine の真の wire-up 未完成 (= Strict と同 output)、
//! diff 0 を確認することで 「Smart 投入で挙動破壊なし」 ベースライン確認。
//!
//! ## 使い方
//!
//! ```bash
//! cargo run --bin furigana-diff-engines -- <corpus.toml> [--rules-dir <dir>] [--core-dict-dir <dir>] [-v]
//! ```
//!
//! ## corpus format
//!
//! TOML、 `[[case]]` array of tables:
//!
//! ```toml
//! [[case]]
//! input = "灰桜"
//! mode = "hiragana"        # hiragana / ruby / tts / romaji / kanji
//! expected = "はいざくら"   # optional、 reference 用 (diff_engines は使わない)
//! note = "桜の品種名"       # optional
//! ```

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use furigana::scoring::candidate::Engine;
use furigana::tts::TtsOptions;
use furigana::Furigana;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "furigana-diff-engines",
    about = "Compare Smart vs Strict engine outputs over a corpus",
    long_about = None,
)]
struct Args {
    /// Corpus TOML file path (= [[case]] arrays)
    corpus: PathBuf,
    /// Optional rules dir (= furigana-dict/rules/)
    #[arg(long)]
    rules_dir: Option<PathBuf>,
    /// Optional core dict dir (= furigana-dict/core/、 複数指定可)
    #[arg(long)]
    core_dict_dir: Vec<PathBuf>,
    /// 全 case 出力 (差分なしも含む、 default は diff のみ)
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Deserialize)]
struct CorpusFile {
    #[serde(default, rename = "case")]
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    input: String,
    #[serde(default = "default_mode")]
    mode: String,
    /// 参照 reference (diff_engines は使わない、 caller の note 用)
    #[serde(default)]
    #[allow(dead_code)]
    expected: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

fn default_mode() -> String {
    "ruby".into()
}

/// Furigana を指定 mode で実行、 出力文字列を返す。
fn run_case(f: &Furigana, input: &str, mode: &str) -> Result<String> {
    Ok(match mode {
        "hiragana" => f.to_hiragana(input),
        "ruby" => f.to_ruby(input),
        "tts" => f.to_tts(input, &TtsOptions::default()),
        "romaji" => f.to_romaji(input, furigana::romaji::RomajiStyle::Hepburn),
        "kanji" => input.to_string(),
        other => return Err(anyhow!("unsupported mode: {:?}", other)),
    })
}

/// Furigana を指定 engine + 共通 args で構築。
fn build_furigana(engine: Engine, args: &Args) -> Result<Furigana> {
    let mut b = Furigana::builder().engine(engine);
    if let Some(rules) = &args.rules_dir {
        b = b.rules_dir(rules);
    }
    for d in &args.core_dict_dir {
        b = b.core_dict_dir(d);
    }
    b.build().context("build Furigana")
}

fn run() -> Result<ExitCode> {
    let args = Args::parse();

    // corpus 読み込み
    let content = fs::read_to_string(&args.corpus)
        .with_context(|| format!("read corpus: {:?}", args.corpus))?;
    let corpus: CorpusFile = toml::from_str(&content).context("parse corpus TOML")?;

    if corpus.cases.is_empty() {
        eprintln!("warning: corpus has no cases, exiting OK");
        return Ok(ExitCode::SUCCESS);
    }

    // 両 engine 構築 (1 度だけ)
    let strict = build_furigana(Engine::Strict, &args)?;
    let smart = build_furigana(Engine::Smart, &args)?;

    let mut diff_count = 0usize;
    let mut error_count = 0usize;

    for (i, case) in corpus.cases.iter().enumerate() {
        let strict_out = match run_case(&strict, &case.input, &case.mode) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error case #{i} (Strict): {e}");
                error_count += 1;
                continue;
            }
        };
        let smart_out = match run_case(&smart, &case.input, &case.mode) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error case #{i} (Smart): {e}");
                error_count += 1;
                continue;
            }
        };

        if strict_out != smart_out {
            diff_count += 1;
            println!("DIFF #{i}: input={:?} mode={:?}", case.input, case.mode);
            println!("  Strict: {strict_out:?}");
            println!("  Smart:  {smart_out:?}");
            if let Some(note) = &case.note {
                println!("  Note:   {note}");
            }
        } else if args.verbose {
            println!(
                "OK   #{i}: input={:?} mode={:?} -> {:?}",
                case.input, case.mode, strict_out
            );
        }
    }

    println!();
    println!("=== Summary ===");
    println!("Total cases: {}", corpus.cases.len());
    println!("Diffs:       {diff_count}");
    println!("Errors:      {error_count}");

    if diff_count > 0 || error_count > 0 {
        Ok(ExitCode::FAILURE)
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:?}");
            ExitCode::FAILURE
        }
    }
}
