//! Shopping list → buy-and-deliver ("Amazon for EVE").
//!
//! Parse an in-game multibuy paste (or a hand-written list) into `(name,
//! quantity)` pairs so the app can price each item across the trade hubs, pick
//! the cheapest source, and hand off a courier. Parsing is pure and
//! unit-tested; the pricing/courier fusion lives in the shell.

/// One requested line item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WantedItem {
    pub name: String,
    pub quantity: i64,
}

/// Parse a multibuy / shopping-list paste into `(name, quantity)` lines,
/// merging duplicate names. Handles the common formats: `Name<TAB>Qty`,
/// `Name xN`, a trailing bare number (`Name 5`), and a plain `Name` (qty 1).
/// Quantities with thousands separators (`1,000`) are tolerated. Pure.
pub fn parse_multibuy(text: &str) -> Vec<WantedItem> {
    let mut order: Vec<String> = Vec::new();
    let mut qty: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let (name, quantity) = split_quantity(line);
        let name = name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        if !qty.contains_key(&name) {
            order.push(name.clone());
        }
        *qty.entry(name).or_insert(0) += quantity.max(1);
    }
    order.into_iter().map(|name| {
        let quantity = qty[&name];
        WantedItem { name, quantity }
    }).collect()
}

/// Split a trailing quantity off a line. Recognizes a tab-separated number, a
/// ` xN` suffix, or a trailing bare integer; defaults to 1.
fn split_quantity(line: &str) -> (&str, i64) {
    // Tab-separated `Name\tQty` (in-game multibuy copy).
    if let Some((name, q)) = line.rsplit_once('\t') {
        if let Some(n) = parse_int(q) {
            return (name, n);
        }
    }
    // ` xN` stack suffix.
    if let Some((name, q)) = line.rsplit_once(" x") {
        if let Some(n) = parse_int(q) {
            return (name, n);
        }
    }
    // Trailing bare integer (`Name 5`) — only when there's a name before it.
    if let Some((name, q)) = line.rsplit_once(' ') {
        if let Some(n) = parse_int(q) {
            if !name.trim().is_empty() {
                return (name, n);
            }
        }
    }
    (line, 1)
}

/// Parse an integer, tolerating surrounding whitespace and thousands commas.
fn parse_int(s: &str) -> Option<i64> {
    let cleaned: String = s.trim().chars().filter(|c| *c != ',').collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<i64>().ok().filter(|n| *n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_formats_and_merges_dupes() {
        let text = "\
Tritanium\t1,000
Pyerite 500
Damage Control II x2
Warrior II
Tritanium\t500";
        let items = parse_multibuy(text);
        assert_eq!(items[0], WantedItem { name: "Tritanium".into(), quantity: 1500 }); // merged
        assert_eq!(items[1], WantedItem { name: "Pyerite".into(), quantity: 500 });
        assert_eq!(items[2], WantedItem { name: "Damage Control II".into(), quantity: 2 });
        assert_eq!(items[3], WantedItem { name: "Warrior II".into(), quantity: 1 });
        assert_eq!(items.len(), 4); // first-seen order preserved, dupe folded
    }

    #[test]
    fn blank_and_garbage_lines_skipped() {
        let items = parse_multibuy("\n  \nRifter\n");
        assert_eq!(items, vec![WantedItem { name: "Rifter".into(), quantity: 1 }]);
    }
}
