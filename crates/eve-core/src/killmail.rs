//! Killmail → fit reconstruction.
//!
//! Given a killmail id + hash (from zKill), the public ESI killmail endpoint
//! returns the victim's fitted items with dogma slot flags. We rebuild the fit
//! as EFT text so it drops straight into the fitting simulator — the plan's
//! "killmail → enemy-fit reconstruction" workflow. Slot mapping and EFT
//! assembly are pure and unit-tested; the fetch rides the cache-first client.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::esi::EsiClient;

/// One victim item off the killmail (only what reconstruction needs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KmItem {
    pub item_type_id: i64,
    /// Dogma location flag (which slot/bay the item sat in).
    pub flag: i64,
    #[serde(default)]
    pub quantity_destroyed: Option<i64>,
    #[serde(default)]
    pub quantity_dropped: Option<i64>,
}

impl KmItem {
    pub fn quantity(&self) -> i64 {
        self.quantity_destroyed.unwrap_or(0) + self.quantity_dropped.unwrap_or(0)
    }
}

/// The killmail victim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KmVictim {
    #[serde(default)]
    pub character_id: Option<i64>,
    pub ship_type_id: i64,
    #[serde(default)]
    pub items: Vec<KmItem>,
}

/// A killmail (ESI `GET /killmails/{id}/{hash}/`, public).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Killmail {
    pub killmail_id: i64,
    #[serde(default)]
    pub solar_system_id: i64,
    pub victim: KmVictim,
}

/// Fitting sections a killmail item can land in, in EFT section order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Slot {
    Low,
    Mid,
    High,
    Rig,
    Subsystem,
    Drone,
    Cargo,
}

/// Map an ESI dogma location flag to a fitting section. `None` = not part of
/// the fit (implants, structure bays, …). Pure.
pub fn slot_of(flag: i64) -> Option<Slot> {
    match flag {
        11..=18 => Some(Slot::Low),
        19..=26 => Some(Slot::Mid),
        27..=34 => Some(Slot::High),
        92..=94 => Some(Slot::Rig),
        125..=128 => Some(Slot::Subsystem),
        87 => Some(Slot::Drone),
        5 => Some(Slot::Cargo),
        _ => None,
    }
}

/// Build EFT text from named, slotted items. Items must carry resolved names;
/// module slots list one line per unit, drones/cargo collapse to `Name xN`.
/// Pure.
pub fn build_eft(ship_name: &str, fit_name: &str, items: &[(Slot, String, i64)]) -> String {
    let mut out = format!("[{ship_name}, {fit_name}]\n");
    for section in [Slot::Low, Slot::Mid, Slot::High, Slot::Rig, Slot::Subsystem] {
        let mut any = false;
        for (slot, name, qty) in items.iter().filter(|(s, _, _)| *s == section) {
            let _ = slot;
            for _ in 0..(*qty).max(1) {
                out.push_str(name);
                out.push('\n');
                any = true;
            }
        }
        if any {
            out.push('\n');
        }
    }
    for (section, _label) in [(Slot::Drone, "drones"), (Slot::Cargo, "cargo")] {
        let mut any = false;
        for (slot, name, qty) in items.iter().filter(|(s, _, _)| *s == section) {
            let _ = slot;
            out.push_str(&format!("{name} x{}\n", (*qty).max(1)));
            any = true;
        }
        if any {
            out.push('\n');
        }
    }
    out.trim_end().to_string() + "\n"
}

/// Fetch a killmail (public, hash-authenticated) over the cache-first client.
#[derive(Clone)]
pub struct KillmailClient {
    esi: EsiClient,
}

impl KillmailClient {
    pub fn new(esi: EsiClient) -> Self {
        Self { esi }
    }

    pub async fn killmail(&self, killmail_id: i64, hash: &str) -> Result<Killmail> {
        let path = format!("/latest/killmails/{killmail_id}/{hash}/");
        self.esi.get_public_json::<Killmail>(&path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_map_to_sections() {
        assert_eq!(slot_of(11), Some(Slot::Low));
        assert_eq!(slot_of(22), Some(Slot::Mid));
        assert_eq!(slot_of(27), Some(Slot::High));
        assert_eq!(slot_of(92), Some(Slot::Rig));
        assert_eq!(slot_of(87), Some(Slot::Drone));
        assert_eq!(slot_of(5), Some(Slot::Cargo));
        assert_eq!(slot_of(89), None); // implant slot — not part of the fit
    }

    #[test]
    fn builds_eft_in_section_order() {
        let items = vec![
            (Slot::High, "200mm AutoCannon II".to_string(), 3),
            (Slot::Low, "Gyrostabilizer II".to_string(), 1),
            (Slot::Mid, "5MN Microwarpdrive II".to_string(), 1),
            (Slot::Drone, "Warrior II".to_string(), 4),
        ];
        let eft = build_eft("Rifter", "Reconstructed", &items);
        assert!(eft.starts_with("[Rifter, Reconstructed]\n"));
        // Lows before mids before highs; three gun lines; drones collapsed.
        let gyro = eft.find("Gyrostabilizer II").unwrap();
        let mwd = eft.find("5MN Microwarpdrive II").unwrap();
        let gun = eft.find("200mm AutoCannon II").unwrap();
        assert!(gyro < mwd && mwd < gun);
        assert_eq!(eft.matches("200mm AutoCannon II").count(), 3);
        assert!(eft.contains("Warrior II x4"));
    }
}
