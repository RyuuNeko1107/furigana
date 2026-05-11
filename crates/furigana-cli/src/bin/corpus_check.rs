//! Corpus regression test runner (★alpha.17、 expected match 計測用)。
//!
//! 形態素辞書 (IPADIC / UniDic) の比較や、 dict 改善前後の精度比較に使う。
//! `[[case]]` array の corpus TOML を input に取り、 各 case を `f.to_*` で実行、
//! `expected` field と照合して正解率を集計する。
//!
//! ## 使い方
//!
//! ```bash
//! cargo run --release --bin furigana-corpus-check -- \
//!     --rules-dir <furigana-dict/rules> \
//!     --core-dict-dir <furigana-dict/core/jukugo> \
//!     --core-dict-dir <furigana-dict/core/unihan> \
//!     --core-dict-dir <furigana-dict/core/kanji> \
//!     <corpus.toml>
//! ```
//!
//! IPADIC / UniDic 比較は同じコマンドを feature flag を変えて 2 回 build し直す:
//!
//! ```bash
//! cargo build --release --bin furigana-corpus-check
//! cargo build --release --bin furigana-corpus-check --no-default-features --features dict-unidic
//! ```

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use furigana::tts::TtsOptions;
use furigana::Furigana;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "furigana-corpus-check",
    about = "Run a corpus and report expected-match rate"
)]
struct Args {
    /// Corpus TOML file path (= [[case]] arrays)
    corpus: PathBuf,
    /// Optional rules dir (= furigana-dict/rules/)
    #[arg(long)]
    rules_dir: Option<PathBuf>,
    /// Optional core dict dir (= furigana-dict/core/*、 複数指定可)
    #[arg(long)]
    core_dict_dir: Vec<PathBuf>,
    /// Print every case (default は failing case のみ)
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
    #[serde(default)]
    expected: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    note: Option<String>,
}

fn default_mode() -> String {
    "ruby".into()
}

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

fn build_furigana(args: &Args) -> Result<Furigana> {
    let mut b = Furigana::builder();
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

    let content = fs::read_to_string(&args.corpus)
        .with_context(|| format!("read corpus: {:?}", args.corpus))?;
    let corpus: CorpusFile = toml::from_str(&content).context("parse corpus TOML")?;

    if corpus.cases.is_empty() {
        eprintln!("warning: corpus has no cases");
        return Ok(ExitCode::SUCCESS);
    }

    let f = build_furigana(&args)?;

    let mut expected_total = 0usize;
    let mut correct = 0usize;
    let mut errors = 0usize;

    for (i, case) in corpus.cases.iter().enumerate() {
        let actual = match run_case(&f, &case.input, &case.mode) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error case #{i}: {e}");
                errors += 1;
                continue;
            }
        };
        if let Some(expected) = &case.expected {
            expected_total += 1;
            let pass = actual == *expected;
            if pass {
                correct += 1;
                if args.verbose {
                    println!("OK   #{i}: {:?} -> {:?}", case.input, actual);
                }
            } else {
                println!("FAIL #{i}: input={:?} mode={:?}", case.input, case.mode);
                println!("  expected: {expected:?}");
                println!("  actual:   {actual:?}");
            }
        }
    }

    println!();
    println!("=== Summary ===");
    println!("Total cases:    {}", corpus.cases.len());
    if expected_total > 0 {
        let pct = (correct * 100) as f64 / expected_total as f64;
        println!("Expected match: {correct}/{expected_total} ({pct:.1}%)");
    }
    if errors > 0 {
        println!("Errors:         {errors}");
    }

    if correct < expected_total || errors > 0 {
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
