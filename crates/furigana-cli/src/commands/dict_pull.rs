//! `furigana dict pull` の実装
//!
//! GitHub Releases から furigana-dict の tarball を取得して
//! `<data_dir>/{core,rules}/` に展開する。
//!
//! 流れ:
//! 1. version 解決 (`--version` 指定 or GitHub API で latest)
//! 2. tarball + sha256 sidecar を download
//! 3. SHA-256 検証
//! 4. 既存 `<data_dir>/{core,rules}/` を削除して tar.gz を展開
//!
//! tarball の中身は `core/...` と `rules/...` の相対パス (release.yml で
//! `tar -czf ARCHIVE core/ rules/` してるため)。展開先は `<data_dir>` 直下。

use crate::paths::Paths;
use anyhow::{anyhow, bail, Context, Result};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const REPO: &str = "RyuuNeko1107/furigana-dict";
const USER_AGENT: &str = concat!("furigana-cli/", env!("CARGO_PKG_VERSION"));

pub fn run(paths: &Paths, version: Option<&str>) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(60))
        .build()
        .context("HTTP クライアント初期化失敗")?;

    let tag = if let Some(v) = version {
        v.to_string()
    } else {
        println!("最新リリースを確認中...");
        resolve_latest_tag(&client)
            .context("最新リリースの解決に失敗 (network or GitHub API エラー)")?
    };
    println!("取得対象: {tag}");

    let archive_name = format!("furigana-dict-{tag}.tar.gz");
    let tarball_url =
        format!("https://github.com/{REPO}/releases/download/{tag}/{archive_name}");
    let sha_url = format!("{tarball_url}.sha256");

    println!("ダウンロード中: {archive_name}");
    let tarball = download_bytes(&client, &tarball_url)
        .with_context(|| format!("tarball 取得失敗: {tarball_url}"))?;
    println!("  {} bytes", tarball.len());

    println!("SHA-256 sidecar 取得中...");
    let expected_hex = match download_text(&client, &sha_url) {
        Ok(text) => parse_sha256_sidecar(&text)
            .with_context(|| format!("sha256 sidecar の parse 失敗: {sha_url}"))?,
        Err(e) => {
            // sidecar が無い古い release もあり得るので warn にとどめる
            tracing::warn!("SHA-256 sidecar 取得失敗: {e}. 検証をスキップします");
            String::new()
        }
    };
    if !expected_hex.is_empty() {
        let actual_hex = sha256_hex(&tarball);
        if !actual_hex.eq_ignore_ascii_case(&expected_hex) {
            bail!(
                "SHA-256 mismatch:\n  expected: {expected_hex}\n  actual:   {actual_hex}"
            );
        }
        println!("SHA-256 検証 OK");
    }

    println!("展開中...");
    extract_to(&tarball, paths).context("tar.gz 展開失敗")?;

    println!("完了: {tag} を {} に配置しました", paths.data_root().display());
    Ok(())
}

/// GitHub API `/releases/latest` から tag_name を取得。
fn resolve_latest_tag(client: &reqwest::blocking::Client) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
    }
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        bail!(
            "GitHub API {status}: {url}\n  body: {}",
            body.chars().take(300).collect::<String>()
        );
    }
    let release: Release = resp.json()?;
    Ok(release.tag_name)
}

fn download_bytes(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>> {
    let resp = client.get(url).send()?;
    let status = resp.status();
    if !status.is_success() {
        bail!("HTTP {status}: {url}");
    }
    Ok(resp.bytes()?.to_vec())
}

fn download_text(client: &reqwest::blocking::Client, url: &str) -> Result<String> {
    let resp = client.get(url).send()?;
    let status = resp.status();
    if !status.is_success() {
        bail!("HTTP {status}: {url}");
    }
    Ok(resp.text()?)
}

/// `sha256sum` 形式 (`<hex>  <filename>`) から hex を抜き出す。
fn parse_sha256_sidecar(text: &str) -> Result<String> {
    let first = text
        .lines()
        .next()
        .ok_or_else(|| anyhow!("空の sha256 sidecar"))?
        .trim();
    let hex = first
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow!("sha256 sidecar の形式が不正: {first:?}"))?;
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("sha256 hex の長さ/文字種が不正: {hex:?}");
    }
    Ok(hex.to_string())
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// tar.gz バイト列を `<data_dir>/data/` 配下に flat 展開する。
///
/// furigana-dict repo の archive は `core/...` `rules/...` で 2 階層に分かれて
/// いるが、配布物は最終的に `data/` 1 階層にまとめる:
/// - `core/unihan.toml`        → `data/unihan.toml`
/// - `core/jukugo/general.toml`→ `data/jukugo/general.toml`
/// - `rules/days.toml`         → `data/days.toml`
/// - `rules/counters/*.toml`   → `data/counters/*.toml`
///
/// lib loader は内部的に「Dict (recursive *.toml で `[entries]` 拾う) vs Rules
/// (特定ファイル名 + counters/ context/ サブのみ)」と排他的に scan するので、
/// 同じ `data/` ディレクトリを両方に渡しても干渉しない (paths.rs 参照)。
///
/// 「core/ と rules/ を分ける必要ない (同じ furigana-dict から PR/DL する
/// データなのに)」という指摘を受けてこの flat layout に統合した。
///
/// 既存の `data_root/` 配下にあった分は **`user/` と `overrides.toml` を残して**
/// 削除してから展開する。これにより古い配布ファイルが残らない一方、ユーザー
/// 追加分は保持される。
fn extract_to(tarball: &[u8], paths: &Paths) -> Result<()> {
    let data_root: PathBuf = paths.data_root();
    let user_dir: PathBuf = paths.dict_user_dir();
    let overrides: PathBuf = paths.overrides_file();

    // 既存の配布ファイルを掃除 (user / overrides は保持)
    if data_root.exists() {
        for entry in fs::read_dir(&data_root)? {
            let path = entry?.path();
            if path == user_dir || path == overrides {
                continue;
            }
            if path.is_dir() {
                fs::remove_dir_all(&path)
                    .with_context(|| format!("既存削除失敗: {}", path.display()))?;
            } else {
                fs::remove_file(&path)
                    .with_context(|| format!("既存削除失敗: {}", path.display()))?;
            }
        }
    } else {
        fs::create_dir_all(&data_root)?;
    }

    let gz = GzDecoder::new(tarball);
    let mut archive = tar::Archive::new(gz);
    archive.set_preserve_permissions(false);
    archive.set_overwrite(true);

    let canonical_root = data_root
        .canonicalize()
        .unwrap_or_else(|_| data_root.clone());

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.into_owned();
        // archive 内 `core/...` `rules/...` の prefix を剥がして `data/` 直下に。
        // prefix だけのディレクトリエントリ (`core/` `rules/`) は rest が空に
        // なるので skip (data_root はすでに作ってある)。
        let dest = if let Ok(rest) = entry_path.strip_prefix("core") {
            if rest.as_os_str().is_empty() {
                continue;
            }
            data_root.join(rest)
        } else if let Ok(rest) = entry_path.strip_prefix("rules") {
            if rest.as_os_str().is_empty() {
                continue;
            }
            data_root.join(rest)
        } else {
            // 想定外の top-level entry は無視 (README 等が混入してもスキップ)
            tracing::debug!("skip archive entry: {}", entry_path.display());
            continue;
        };
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        // path traversal 防御: dest の親が data_root 配下に収まることを確認
        let canonical_parent = dest
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .unwrap_or_else(|| dest.clone());
        if !canonical_parent.starts_with(&canonical_root) {
            bail!(
                "path traversal を検出: {} は {} の外",
                entry_path.display(),
                data_root.display()
            );
        }
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&dest)?;
        } else {
            entry.unpack(&dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sha256_sidecar() {
        let text = "8e7d1c4...abcd  furigana-dict-v0.1.0.tar.gz\n";
        // 短すぎるので reject
        assert!(parse_sha256_sidecar(text).is_err());

        let valid = format!("{}  furigana-dict-v0.1.0.tar.gz\n", "a".repeat(64));
        assert_eq!(parse_sha256_sidecar(&valid).unwrap(), "a".repeat(64));
    }

    #[test]
    fn sha256_known_vector() {
        // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

