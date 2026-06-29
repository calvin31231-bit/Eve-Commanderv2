//! Parse EVE's inventory "copy to clipboard" text into item lines.
//!
//! Selecting items in an in-game inventory and copying yields tab-separated rows
//! — `Name\tQuantity\tGroup\t…`. This parser pulls `(name, quantity)` from that
//! (tolerant of thousands separators and rows with no quantity column), summing
//! duplicates. Pure; name→price valuation happens in the command layer.

/// One parsed loot line.
#[derive(Debug, Clone, PartialEq)]
pub struct LootLine {
    pub name: String,
    pub quantity: i64,
}

/// Parse a digit string that may carry thousands separators ("1,000", "1 050").
/// Returns 1 for an empty/garbage quantity so a bare name still counts as one.
fn parse_qty(s: &str) -> i64 {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.parse::<i64>() {
        Ok(n) if n > 0 => n,
        _ => 1,
    }
}

/// Parse pasted inventory text into deduplicated `(name, quantity)` lines,
/// summing repeats (case-insensitive name match). Pure.
pub fn parse_loot(text: &str) -> Vec<LootLine> {
    let mut out: Vec<LootLine> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        // Tab-separated when copied from the client; fall back to the whole line
        // as the name with quantity 1.
        let mut cols = line.split('\t');
        let name = cols.next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        let quantity = match cols.next() {
            Some(q) if !q.trim().is_empty() => parse_qty(q),
            _ => 1,
        };
        match out.iter_mut().find(|l| l.name.eq_ignore_ascii_case(name)) {
            Some(existing) => existing.quantity += quantity,
            None => out.push(LootLine { name: name.to_string(), quantity }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tab_separated_inventory_paste() {
        let text = "Nanite Repair Paste\t50\tCommodity\nCaldari Navy Ballistic Control System\t1\tModule\nTritanium\t1,000";
        let lines = parse_loot(text);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], LootLine { name: "Nanite Repair Paste".into(), quantity: 50 });
        assert_eq!(lines[1].name, "Caldari Navy Ballistic Control System");
        assert_eq!(lines[2], LootLine { name: "Tritanium".into(), quantity: 1000 });
    }

    #[test]
    fn dedups_and_defaults_quantity() {
        let text = "Tritanium\t100\nTritanium\t50\nDamage Control II";
        let lines = parse_loot(text);
        assert_eq!(lines.len(), 2);
        let trit = lines.iter().find(|l| l.name == "Tritanium").unwrap();
        assert_eq!(trit.quantity, 150);
        // Bare name with no quantity column → 1.
        let dc = lines.iter().find(|l| l.name == "Damage Control II").unwrap();
        assert_eq!(dc.quantity, 1);
    }

    #[test]
    fn ignores_blank_lines() {
        assert!(parse_loot("\n\n   \n").is_empty());
    }
}
