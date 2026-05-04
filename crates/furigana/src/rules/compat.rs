//! 異体字マップ (compat_map.tsv)
//!
//! 異体字 → 標準字 の正規化テーブル。
//! 例: 髙→高, 﨑→崎, 德→徳。
//! 形態素解析・辞書ルックアップに先立って入力テキストを正規化することで、
//! 同義表記揺れを 1 つの読みエントリにまとめられる。
//!
//! ## 例 (TSV: 異体字\t標準字)
//! ```text
//! 髙	高
//! 﨑	崎
//! 德	徳
//! ```

use std::collections::HashMap;

/// compat_map.tsv 1 行
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatEntry {
    /// 異体字 (1 文字を想定)
    pub variant: String,
    /// 標準字
    pub canonical: String,
}

/// compat_map.tsv 全体
///
/// `entries` は記載順を保持し、`map` は高速 lookup 用キャッシュ。
/// ロード時に同期される。
#[derive(Debug, Default, Clone)]
pub struct CompatData {
    /// エントリ列 (記載順)
    pub entries: Vec<CompatEntry>,
    /// variant → canonical の HashMap (lookup 用)
    pub map: HashMap<String, String>,
}

impl CompatData {
    /// `entries` から `map` を再構築する。ロード後に呼ぶ。
    pub fn rebuild_map(&mut self) {
        self.map = self
            .entries
            .iter()
            .map(|e| (e.variant.clone(), e.canonical.clone()))
            .collect();
    }

    /// 異体字を標準字に変換 (該当無しなら None)
    #[must_use]
    pub fn lookup(&self, variant: &str) -> Option<&str> {
        self.map.get(variant).map(String::as_str)
    }

    /// 件数
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 空判定
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_map_syncs_lookup() {
        let mut d = CompatData {
            entries: vec![
                CompatEntry {
                    variant: "髙".into(),
                    canonical: "高".into(),
                },
                CompatEntry {
                    variant: "﨑".into(),
                    canonical: "崎".into(),
                },
            ],
            map: HashMap::new(),
        };
        // map 未構築だと lookup は失敗
        assert_eq!(d.lookup("髙"), None);
        d.rebuild_map();
        assert_eq!(d.lookup("髙"), Some("高"));
        assert_eq!(d.lookup("﨑"), Some("崎"));
        assert_eq!(d.lookup("高"), None);
    }

    #[test]
    fn default_is_empty() {
        let d = CompatData::default();
        assert!(d.is_empty());
    }
}
