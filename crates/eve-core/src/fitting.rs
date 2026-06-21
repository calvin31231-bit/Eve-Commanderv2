//! Ship fitting: EFT-format import/export.
//!
//! EFT (EVE Fitting Tool) text is the lingua franca for sharing fits across the
//! ecosystem (PYFA, the in-game fitting window, killboards). A block looks like:
//!
//! ```text
//! [Rifter, Shield Kite]
//! Damage Control II
//! 200mm AutoCannon II, Republic Fleet EMP S
//!
//! 5MN Y-T8 Compact Microwarpdrive
//!
//! Hobgoblin II x5
//! Nanite Repair Paste x50
//! ```
//!
//! The first line is `[ShipType, FitName]`; remaining non-blank lines are
//! modules (an optional `, Charge` loaded, an optional `xN` stack count for
//! drones/cargo). Slot *classification* needs dogma data (a later, SDE-backed
//! step); this layer captures the structure losslessly. Parsing and re-emitting
//! are pure and unit-tested.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::sde::Sde;

/// One line of a fit: a module/drone/charge with optional loaded charge and
/// stack quantity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitItem {
    pub name: String,
    /// A charge loaded into the module (`Module, Charge` syntax), if any.
    pub charge: Option<String>,
    /// Stack count (`xN` suffix). 1 for single fitted modules.
    pub quantity: i64,
}

/// A parsed ship fitting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fit {
    pub ship: String,
    pub name: String,
    pub items: Vec<FitItem>,
}

impl Fit {
    /// Distinct module/charge/drone type names referenced (ship + items +
    /// charges), de-duplicated in first-seen order. Useful for batch name→id
    /// resolution (e.g. "can I fly this?").
    pub fn type_names(&self) -> Vec<String> {
        let mut seen = Vec::new();
        let mut push = |n: &str| {
            if !n.is_empty() && !seen.iter().any(|s: &String| s == n) {
                seen.push(n.to_string());
            }
        };
        push(&self.ship);
        for it in &self.items {
            push(&it.name);
            if let Some(c) = &it.charge {
                push(c);
            }
        }
        seen
    }
}

/// Parse an EFT block into a [`Fit`]. Returns `None` if the header line isn't a
/// well-formed `[Ship, Name]`. Tolerant of blank lines and surrounding
/// whitespace.
pub fn parse_eft(text: &str) -> Option<Fit> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let header = lines.next()?;
    let inner = header.strip_prefix('[')?.strip_suffix(']')?;
    let (ship, name) = inner.split_once(',')?;
    let ship = ship.trim();
    if ship.is_empty() {
        return None;
    }

    let mut items = Vec::new();
    for line in lines {
        items.push(parse_item(line));
    }
    Some(Fit {
        ship: ship.to_string(),
        name: name.trim().to_string(),
        items,
    })
}

/// Parse one module line: split a trailing `xN` quantity and an optional
/// `, Charge`.
fn parse_item(line: &str) -> FitItem {
    // Trailing stack count: "Hobgoblin II x5".
    let (body, quantity) = match line.rsplit_once(" x") {
        Some((head, tail)) if tail.chars().all(|c| c.is_ascii_digit()) && !tail.is_empty() => {
            (head.trim(), tail.parse::<i64>().unwrap_or(1).max(1))
        }
        _ => (line, 1),
    };
    // Optional loaded charge: "200mm AutoCannon II, Republic Fleet EMP S".
    let (name, charge) = match body.split_once(',') {
        Some((m, c)) => (m.trim().to_string(), Some(c.trim().to_string())),
        None => (body.to_string(), None),
    };
    FitItem { name, charge, quantity }
}

/// Serialize a [`Fit`] back to EFT text (header + one line per item).
pub fn to_eft(fit: &Fit) -> String {
    let mut out = format!("[{}, {}]\n", fit.ship, fit.name);
    for it in &fit.items {
        out.push_str(&it.name);
        if let Some(c) = &it.charge {
            out.push_str(", ");
            out.push_str(c);
        }
        if it.quantity > 1 {
            out.push_str(&format!(" x{}", it.quantity));
        }
        out.push('\n');
    }
    out
}

/// A fit item resolved against the SDE (type id filled when the name is known).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedItem {
    pub type_id: Option<i64>,
    pub name: String,
    pub charge: Option<String>,
    pub quantity: i64,
}

/// A fit with its ship and items resolved to type ids where the SDE knows them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedFit {
    pub ship_type_id: Option<i64>,
    pub ship: String,
    pub name: String,
    pub items: Vec<ResolvedItem>,
    /// Names the SDE couldn't resolve (needs the full prebuilt SDE).
    pub unresolved: Vec<String>,
}

/// SDE-backed fit resolution (EFT text → typed items).
#[derive(Clone)]
pub struct FittingClient {
    sde: Sde,
}

impl FittingClient {
    pub fn new(sde: Sde) -> Self {
        Self { sde }
    }

    /// Parse EFT text and resolve every referenced name to a type id via the
    /// SDE. Returns `None` only when the header itself is malformed.
    pub async fn resolve_eft(&self, text: &str) -> Result<Option<ResolvedFit>> {
        let Some(fit) = parse_eft(text) else {
            return Ok(None);
        };
        let ship_type_id = self.sde.type_id_by_name(&fit.ship).await?;
        let mut unresolved = Vec::new();
        if ship_type_id.is_none() {
            unresolved.push(fit.ship.clone());
        }

        let mut items = Vec::with_capacity(fit.items.len());
        for it in &fit.items {
            let type_id = self.sde.type_id_by_name(&it.name).await?;
            if type_id.is_none() && !unresolved.contains(&it.name) {
                unresolved.push(it.name.clone());
            }
            items.push(ResolvedItem {
                type_id,
                name: it.name.clone(),
                charge: it.charge.clone(),
                quantity: it.quantity,
            });
        }
        Ok(Some(ResolvedFit {
            ship_type_id,
            ship: fit.ship,
            name: fit.name,
            items,
            unresolved,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EFT: &str = "[Rifter, Shield Kite]\nDamage Control II\n200mm AutoCannon II, Republic Fleet EMP S\n\n5MN Y-T8 Compact Microwarpdrive\n\nHobgoblin II x5\nNanite Repair Paste x50\n";

    #[test]
    fn parses_header_modules_charges_and_quantities() {
        let fit = parse_eft(EFT).unwrap();
        assert_eq!(fit.ship, "Rifter");
        assert_eq!(fit.name, "Shield Kite");
        assert_eq!(fit.items.len(), 5);

        // Module with a loaded charge.
        let gun = &fit.items[1];
        assert_eq!(gun.name, "200mm AutoCannon II");
        assert_eq!(gun.charge.as_deref(), Some("Republic Fleet EMP S"));
        assert_eq!(gun.quantity, 1);

        // Drone with a stack count.
        let drone = &fit.items[3];
        assert_eq!(drone.name, "Hobgoblin II");
        assert_eq!(drone.quantity, 5);
        assert!(drone.charge.is_none());
    }

    #[test]
    fn rejects_malformed_header() {
        assert!(parse_eft("Damage Control II\n").is_none());
        assert!(parse_eft("[NoComma]\n").is_none());
        assert!(parse_eft("[, NoShip]\n").is_none());
    }

    #[test]
    fn roundtrips_through_eft() {
        let fit = parse_eft(EFT).unwrap();
        let reparsed = parse_eft(&to_eft(&fit)).unwrap();
        assert_eq!(fit, reparsed);
    }

    #[test]
    fn type_names_dedupes_in_order() {
        let fit = parse_eft("[Rifter, Twin]\n200mm AutoCannon II, EMP S\n200mm AutoCannon II, EMP S\n").unwrap();
        // Ship + gun + charge, each once.
        assert_eq!(fit.type_names(), vec!["Rifter", "200mm AutoCannon II", "EMP S"]);
    }

    #[test]
    fn does_not_split_x_in_module_names() {
        // "x" inside a name without a numeric suffix must not be treated as a count.
        let item = parse_item("Festival Launcher");
        assert_eq!(item.name, "Festival Launcher");
        assert_eq!(item.quantity, 1);
    }

    #[tokio::test]
    async fn resolves_known_names_and_flags_unknown() {
        let sde = Sde::open_in_memory().await.unwrap();
        sde.insert_type(587, "Rifter", None).await.unwrap();
        sde.insert_type(2185, "Damage Control II", None).await.unwrap();
        let client = FittingClient::new(sde);

        let fit = client
            .resolve_eft("[Rifter, Test]\nDamage Control II\nMade Up Module\n")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fit.ship_type_id, Some(587));
        assert_eq!(fit.items[0].type_id, Some(2185));
        assert_eq!(fit.items[1].type_id, None);
        assert_eq!(fit.unresolved, vec!["Made Up Module"]);
    }
}
