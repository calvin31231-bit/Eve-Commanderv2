//! Cosmic-signature parsing for the wormhole chain tracker.
//!
//! The in-game probe scanner "copy to clipboard" yields tab-separated rows:
//! `ID \t Group \t Category \t Name \t Signal% \t Distance`. This pulls the
//! stable parts — id, category, name — so a chain can be logged and annotated.
//! Pure; persistence lives in `db::signatures`.

/// One scanned signature (the fields we keep).
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedSig {
    /// The signature id, e.g. "ABC-123".
    pub sig_id: String,
    /// Category: Wormhole / Relic / Data / Gas / Combat / (empty if unknown).
    pub category: String,
    /// Specific name, when resolved in-game.
    pub name: String,
}

/// Normalise a category cell to a short bucket. Pure.
fn category_of(cell: &str) -> String {
    let c = cell.to_ascii_lowercase();
    if c.contains("wormhole") {
        "Wormhole".into()
    } else if c.contains("relic") {
        "Relic".into()
    } else if c.contains("data") {
        "Data".into()
    } else if c.contains("gas") {
        "Gas".into()
    } else if c.contains("combat") {
        "Combat".into()
    } else if c.contains("ore") {
        "Ore".into()
    } else {
        cell.trim().to_string()
    }
}

/// Parse pasted probe-scanner text into signatures. Skips non-signature rows and
/// dedups by id (keeping the most-resolved row). Pure.
pub fn parse_signatures(text: &str) -> Vec<ScannedSig> {
    let mut out: Vec<ScannedSig> = Vec::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        let sig_id = cols.first().map(|s| s.trim()).unwrap_or("");
        // Signature ids look like "ABC-123"; require the dashed form.
        if sig_id.len() < 5 || !sig_id.contains('-') {
            continue;
        }
        let category = cols.get(2).map(|s| category_of(s)).unwrap_or_default();
        let name = cols.get(3).map(|s| s.trim().to_string()).unwrap_or_default();
        match out.iter_mut().find(|s| s.sig_id.eq_ignore_ascii_case(sig_id)) {
            Some(existing) => {
                // Prefer the row that resolved a category/name.
                if existing.category.is_empty() && !category.is_empty() {
                    existing.category = category.clone();
                }
                if existing.name.is_empty() && !name.is_empty() {
                    existing.name = name.clone();
                }
            }
            None => out.push(ScannedSig { sig_id: sig_id.to_string(), category, name }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_probe_scanner_rows() {
        let text = "ABC-123\tCosmic Signature\tWormhole\tUnstable Wormhole\t100.0%\t1.5 AU\n\
                    DEF-456\tCosmic Signature\tRelic Site\tRuined Sansha\t42.0%\t3.1 AU\n\
                    GHI-789\tCosmic Signature\t\t\t12.0%\t-";
        let sigs = parse_signatures(text);
        assert_eq!(sigs.len(), 3);
        assert_eq!(sigs[0].sig_id, "ABC-123");
        assert_eq!(sigs[0].category, "Wormhole");
        assert_eq!(sigs[1].category, "Relic");
        // Unresolved category stays empty.
        assert_eq!(sigs[2].category, "");
    }

    #[test]
    fn ignores_non_signature_lines() {
        assert!(parse_signatures("just some text\nno tabs here").is_empty());
    }

    #[test]
    fn dedups_and_fills_in() {
        // Same sig scanned twice; second pass resolved the category.
        let text = "ABC-123\tCosmic Signature\t\t\t10%\t5 AU\n\
                    ABC-123\tCosmic Signature\tData Site\tSerpentis Hub\t100%\t0";
        let sigs = parse_signatures(text);
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].category, "Data");
        assert_eq!(sigs[0].name, "Serpentis Hub");
    }
}
