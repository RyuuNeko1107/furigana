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
use std::path::Path;
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
    extract_to(&tarball, &paths.data_dir).context("tar.gz 展開失敗")?;

    println!("完了: {tag} を {} に配置しました", paths.data_dir.display());
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

/// tar.gz バイト列を `data_dir` 配下に rebase 展開。
///
/// archive 内のエントリは furigana-dict repo の構造そのまま (`core/...` /
/// `rules/...`)。一方エンジン側 (paths.rs / build_furigana) は:
/// - 語彙辞書を `<data_dir>/dict/core/`
/// - エンジンルールを `<data_dir>/rules/`
/// から読むため、`core/` のみ `dict/core/` にプレフィックスを付け替えて配置する。
///
/// 既存の `<data_dir>/dict/core/` と `<data_dir>/rules/` は先に削除する
/// (古いファイルが残らないように)。ユーザー追加の `<data_dir>/dict/user/` や
/// `<data_dir>/dict/overrides.toml` は別パスなので影響なし。
fn extract_to(tarball: &[u8], data_dir: &Path) -> Result<()> {
    let dict_core = data_dir.join("dict").join("core");
    let rules_dir = data_dir.join("rules");
    for p in [&dict_core, &rules_dir] {
        if p.exists() {
            fs::remove_dir_all(p)
                .with_context(|| format!("既存削除失敗: {}", p.display()))?;
        }
    }
    fs::create_dir_all(&dict_core)?;
    fs::create_dir_all(&rules_dir)?;

    let gz = GzDecoder::new(tarball);
    let mut archive = tar::Archive::new(gz);
    archive.set_preserve_permissions(false);
    archive.set_overwrite(true);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.into_owned();
        let dest = if let Ok(rest) = entry_path.strip_prefix("core") {
            data_dir.join("dict").join("core").join(rest)
        } else if let Ok(rest) = entry_path.strip_prefix("rules") {
            data_dir.join("rules").join(rest)
        } else {
            // 想定外の top-level entry は無視 (README 等が混入してもスキップ)
            tracing::debug!("skip archive entry: {}", entry_path.display());
            continue;
        };
        // path traversal 防御: dest が data_dir 配下に収まることを確認
        let canonical_root = data_dir.canonicalize().unwrap_or_else(|_| data_dir.to_path_buf());
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let canonical_parent = dest
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .unwrap_or_else(|| dest.clone());
        if !canonical_parent.starts_with(&canonical_root) {
            bail!(
                "path traversal を検出: {} は {} の外",
                entry_path.display(),
                data_dir.display()
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

