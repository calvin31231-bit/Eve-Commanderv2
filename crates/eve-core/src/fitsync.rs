//! In-game fitting sync (`esi-fittings` read/write) — the Phase 3 residual.
//!
//! Pull: `GET /characters/{id}/fittings/` returns saved fittings with dogma
//! slot flags; we rebuild EFT via the killmail slot mapping so they import into
//! the local library. Push: EFT's section order is fixed (low, mid, high, rig,
//! then drones/cargo `xN` blocks), so a section-aware parse assigns each module
//! a concrete slot flag and `POST` saves it to the in-game fitting window. The
//! EULA-sanctioned write path — ESI, not input automation. Parsing/mapping are
//! pure and unit-tested.

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::Result;
use crate::esi::EsiClient;
use crate::killmail::Slot;

/// One item in an ESI fitting (both directions use this shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameFitItem {
    pub flag: i64,
    pub quantity: i64,
    pub type_id: i64,
}

/// A saved in-game fitting (ESI `GET /characters/{id}/fittings/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameFitting {
    pub fitting_id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub ship_type_id: i64,
    #[serde(default)]
    pub items: Vec<GameFitItem>,
}

/// A section-parsed EFT fit ready for slot-flag assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionedFit {
    pub ship: String,
    pub name: String,
    /// `(slot, module name, quantity)` — modules qty 1 per line; drones/cargo
    /// carry their `xN`.
    pub items: Vec<(Slot, String, i64)>,
}

/// Parse EFT with section structure intact. EFT's contract: header line, then
/// blank-line-separated sections in low → mid → high → rig order; any later
/// sections whose lines end in ` xN` are drones/cargo. `[Empty … slot]`
/// placeholders are skipped. Pure.
pub fn parse_eft_sections(text: &str) -> Option<SectionedFit> {
    let mut lines = text.trim().lines();
    let header = lines.next()?.trim();
    let inner = header.strip_prefix('[')?.strip_suffix(']')?;
    let (ship, name) = match inner.split_once(',') {
        Some((s, n)) => (s.trim().to_string(), n.trim().to_string()),
        None => (inner.trim().to_string(), "Imported".to_string()),
    };
    if ship.is_empty() {
        return None;
    }

    // Chunk the body into blank-line-separated sections.
    let mut sections: Vec<Vec<&str>> = vec![Vec::new()];
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            if !sections.last().is_some_and(|s| s.is_empty()) {
                sections.push(Vec::new());
            }
        } else {
            sections.last_mut().unwrap().push(line);
        }
    }
    sections.retain(|s| !s.is_empty());

    const MODULE_ORDER: [Slot; 4] = [Slot::Low, Slot::Mid, Slot::High, Slot::Rig];
    let mut items: Vec<(Slot, String, i64)> = Vec::new();
    let mut module_section = 0usize;
    for section in &sections {
        // A section where every line carries an `xN` stack is drones/cargo.
        let all_stacked = section.iter().all(|l| split_stack(l).1 > 1 || l.rsplit_once(" x").is_some_and(|(_, n)| n.parse::<i64>().is_ok()));
        if all_stacked && module_section >= 1 {
            for line in section {
                let (name, qty) = split_stack(line);
                if !name.is_empty() {
                    // First stacked section = drones, later ones = cargo.
                    items.push((Slot::Drone, name, qty));
                }
            }
            continue;
        }
        let slot = *MODULE_ORDER.get(module_section)?;
        module_section += 1;
        for line in section {
            if line.to_lowercase().starts_with("[empty") {
                continue;
            }
            // Strip a loaded charge ("Module, Charge") — the module is what fits.
            let module = line.split(',').next().unwrap_or(line).trim();
            let (name, qty) = split_stack(module);
            if !name.is_empty() {
                items.push((slot, name, qty));
            }
        }
    }
    Some(SectionedFit { ship, name, items })
}

/// Split a trailing ` xN` stack suffix. Pure.
fn split_stack(line: &str) -> (String, i64) {
    if let Some((name, n)) = line.rsplit_once(" x") {
        if let Ok(qty) = n.trim().parse::<i64>() {
            return (name.trim().to_string(), qty.max(1));
        }
    }
    (line.trim().to_string(), 1)
}

/// The concrete dogma flag for the `index`-th module in a section. Pure.
pub fn flag_for(slot: Slot, index: usize) -> i64 {
    let i = index as i64;
    match slot {
        Slot::Low => 11 + i.min(7),
        Slot::Mid => 19 + i.min(7),
        Slot::High => 27 + i.min(7),
        Slot::Rig => 92 + i.min(2),
        Slot::Subsystem => 125 + i.min(3),
        Slot::Drone => 87,
        Slot::Cargo => 5,
    }
}

/// Authenticated fitting reads/writes over the cache-first ESI client.
#[derive(Clone)]
pub struct FitSyncClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl FitSyncClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The character's in-game saved fittings.
    pub async fn fittings(&self, character_id: i64) -> Result<Vec<GameFitting>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/characters/{character_id}/fittings/");
        self.esi.get_auth_json::<Vec<GameFitting>>(&path, &token).await
    }

    /// Save a fitting in-game; returns the new fitting id.
    pub async fn save_fitting(
        &self,
        character_id: i64,
        name: &str,
        description: &str,
        ship_type_id: i64,
        items: &[GameFitItem],
    ) -> Result<i64> {
        #[derive(Deserialize)]
        struct Created {
            fitting_id: i64,
        }
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/characters/{character_id}/fittings/");
        let body = serde_json::json!({
            "name": name,
            "description": description,
            "ship_type_id": ship_type_id,
            "items": items,
        });
        let created: Created = self.esi.post_auth_json(&path, &body, &token).await?;
        Ok(created.fitting_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EFT: &str = "\
[Rifter, Autocannon Rifter]
Gyrostabilizer II
Damage Control II

5MN Microwarpdrive II
Warp Scrambler II

200mm AutoCannon II, EMP S
200mm AutoCannon II, EMP S

Small Projectile Burst Aerator I

Warrior II x3

Nanite Repair Paste x25";

    #[test]
    fn parses_sections_in_eft_order() {
        let fit = parse_eft_sections(EFT).unwrap();
        assert_eq!(fit.ship, "Rifter");
        assert_eq!(fit.name, "Autocannon Rifter");
        // Lows, mids, highs, rig land in their sections; charges stripped.
        assert!(fit.items.contains(&(Slot::Low, "Gyrostabilizer II".into(), 1)));
        assert!(fit.items.contains(&(Slot::Mid, "Warp Scrambler II".into(), 1)));
        assert_eq!(
            fit.items.iter().filter(|(s, n, _)| *s == Slot::High && n == "200mm AutoCannon II").count(),
            2
        );
        assert!(fit.items.contains(&(Slot::Rig, "Small Projectile Burst Aerator I".into(), 1)));
        // Stacked sections (drones + paste) collapse to Drone-bay entries.
        assert!(fit.items.contains(&(Slot::Drone, "Warrior II".into(), 3)));
    }

    #[test]
    fn flags_assign_sequential_slots() {
        assert_eq!(flag_for(Slot::Low, 0), 11);
        assert_eq!(flag_for(Slot::Low, 1), 12);
        assert_eq!(flag_for(Slot::Mid, 0), 19);
        assert_eq!(flag_for(Slot::High, 7), 34);
        assert_eq!(flag_for(Slot::Rig, 2), 94);
        assert_eq!(flag_for(Slot::Drone, 5), 87);
    }

    #[test]
    fn malformed_header_is_none() {
        assert!(parse_eft_sections("no header here").is_none());
        assert!(parse_eft_sections("[ , ]").is_none());
    }
}
