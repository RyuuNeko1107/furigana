//! 後処理ルール (regex ベースの mode 別置換)
//!
//! [`crate::Furigana::to_hiragana`] / `to_ruby` / `to_tts` / `to_romaji` の
//! 出力直前に適用される regex 置換ルール (Step 7 の正規表現ベース置換)。
//!
//! ## ユースケース
//!
//! - 文字列レベルの最終調整 (「ゴジュウパー → ゴジュッパー」等の促音化補正)
//! - mode 別の挙動分岐 (`tts` のみで句読点後の半角スペース除去 等)
//! - 辞書 / context rule で表現しづらい conjugation の手当て
//!
//! ## TOML 形式
//!
//! ```toml
//! [[rule]]
//! pattern = "ジュウパー"
//! replacement = "ジュッパー"
//! modes = ["hiragana", "tts", "romaji"]   # 空 or 省略 = すべての mode
//!
//! [[rule]]
//! pattern = "(\\d+)\\s*ヶ"
//! replacement = "$1カ"   # キャプチャグループ参照可
//! ```
//!
//! ## ステータス
//!
//! Phase 1: 構造定義 + apply 実装。`furigana-dict/rules/postprocess.toml` が
//! 存在しなければ no-op。Phase 2 で具体的な rule を seed 投入する。

use serde::Deserialize;

/// 後処理ルール TOML 全体
///
/// `[[rule]]` array をそのまま受ける defensive な型。pattern parse は
/// 構築時に実行 ([`Self::from_spec`])、生 string のまま `apply` で都度
/// コンパイルしないことで重複コストを避ける。
#[derive(Debug, Default, Clone)]
pub struct PostProcessData {
    rules: Vec<CompiledRule>,
}

/// コンパイル済みの単一ルール (内部表現)
#[derive(Debug, Clone)]
struct CompiledRule {
    re: regex::Regex,
    replacement: String,
    /// 空なら全 mode に適用、空でなければ含まれる mode にのみ適用
    modes: Vec<String>,
}

/// TOML から直接読む型 (deserialize 用)
#[derive(Debug, Default, Deserialize)]
pub struct PostProcessSpec {
    /// `[[rule]]` の集合。 loader が複数 file を merge する際に直接 extend する用途で公開。
    #[serde(default, rename = "rule")]
    pub rules: Vec<PostProcessRuleSpec>,
}

/// TOML の `[[rule]]` 1 件分
#[derive(Debug, Deserialize)]
pub struct PostProcessRuleSpec {
    pub pattern: String,
    pub replacement: String,
    #[serde(default)]
    pub modes: Vec<String>,
}

impl PostProcessData {
    /// `PostProcessSpec` から構築 (regex pre-compile、不正パターンは Err)
    ///
    /// # Errors
    /// regex パターンのコンパイル失敗時。
    pub fn from_spec(spec: PostProcessSpec) -> Result<Self, regex::Error> {
        let rules = spec
            .rules
            .into_iter()
            .map(|r| {
                // size_limit で巨大 regex (compile 後 DFA / NFA size) を拒否。
                // postprocess.toml の pattern は dist (or user) 制御の data なので、
                // 攻撃者が regex bomb を仕込んで memory を食わせるのを防御。
                // 10 MB は通常 pattern (数百 byte) を十分カバー。
                let re = regex::RegexBuilder::new(&r.pattern)
                    .size_limit(10 * 1024 * 1024)
                    .build()?;
                Ok(CompiledRule {
                    re,
                    replacement: r.replacement,
                    modes: r.modes,
                })
            })
            .collect::<Result<Vec<_>, regex::Error>>()?;
        Ok(Self { rules })
    }

    /// 該当 mode の rule を上から順に適用
    ///
    /// rules が空なら text をそのまま返す (no-op)。
    /// `mode` は `"hiragana"` / `"ruby"` / `"tts"` / `"romaji"` 等の文字列。
    #[must_use]
    pub fn apply(&self, text: &str, mode: &str) -> String {
        if self.rules.is_empty() {
            return text.to_string();
        }
        let mut current = text.to_string();
        for r in &self.rules {
            if r.modes.is_empty() || r.modes.iter().any(|m| m == mode) {
                current =
                    r.re.replace_all(&current, r.replacement.as_str())
                        .into_owned();
            }
        }
        current
    }

    /// rule 件数 (debug 用)
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// 空判定
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_no_op() {
        let p = PostProcessData::default();
        assert_eq!(p.apply("こんにちは", "hiragana"), "こんにちは");
    }

    #[test]
    fn applies_simple_replacement() {
        let spec = PostProcessSpec {
            rules: vec![PostProcessRuleSpec {
                pattern: r"ジュウパー".to_string(),
                replacement: "ジュッパー".to_string(),
                modes: vec![],
            }],
        };
        let p = PostProcessData::from_spec(spec).unwrap();
        assert_eq!(
            p.apply("ゴジュウパーセント", "hiragana"),
            "ゴジュッパーセント"
        );
    }

    #[test]
    fn mode_filter_excludes_other_modes() {
        let spec = PostProcessSpec {
            rules: vec![PostProcessRuleSpec {
                pattern: r"AAA".to_string(),
                replacement: "BBB".to_string(),
                modes: vec!["tts".to_string()],
            }],
        };
        let p = PostProcessData::from_spec(spec).unwrap();
        assert_eq!(p.apply("AAA", "tts"), "BBB");
        assert_eq!(p.apply("AAA", "hiragana"), "AAA"); // mode 違うので no-op
    }

    #[test]
    fn capture_group_reference_works() {
        let spec = PostProcessSpec {
            rules: vec![PostProcessRuleSpec {
                pattern: r"(\d+)ヶ".to_string(),
                replacement: "$1カ".to_string(),
                modes: vec![],
            }],
        };
        let p = PostProcessData::from_spec(spec).unwrap();
        assert_eq!(p.apply("3ヶ月", "hiragana"), "3カ月");
    }

    #[test]
    fn invalid_pattern_errors() {
        let spec = PostProcessSpec {
            rules: vec![PostProcessRuleSpec {
                pattern: r"[invalid".to_string(),
                replacement: "X".to_string(),
                modes: vec![],
            }],
        };
        assert!(PostProcessData::from_spec(spec).is_err());
    }
}
