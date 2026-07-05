//! Curated hub-trade item universe.
//!
//! The hub market-analysis spreadsheets ship a hand-picked list of liquid,
//! high-value items worth checking across trade hubs (T2 modules, drones,
//! capital modules, common consumables). We keep that universe as *names*, not
//! type ids, so it survives SDE version drift — names resolve to ids at scan
//! time. The list is intentionally biased toward items with real inter-hub
//! spread and daily volume; it is not exhaustive (the scanner also accepts a
//! user-supplied list).

/// The eight refined minerals (canonical type ids) — the "minerals only" scope.
/// Ids are stable, so no SDE resolution is needed.
pub const MINERALS: &[i64] = &[
    34,    // Tritanium
    35,    // Pyerite
    36,    // Mexallon
    37,    // Isogen
    38,    // Nocxium
    39,    // Zydrine
    40,    // Megacyte
    11399, // Morphite
];

/// The base variants of the main mineable ores (canonical type ids), for the
/// "what should I mine now?" ISK/m³ ranking. Ids are stable. Only the T1 base
/// rock of each family is listed; the +5%/+10% variants track the same minerals.
pub const ORES: &[i64] = &[
    1230,  // Veldspar
    1228,  // Scordite
    1224,  // Pyroxeres
    18,    // Plagioclase
    1227,  // Omber
    20,    // Kernite
    1226,  // Jaspet
    1231,  // Hemorphite
    21,    // Hedbergite
    1229,  // Gneiss
    1232,  // Dark Ochre
    19,    // Spodumain
    1225,  // Crokite
    1223,  // Bistot
    22,    // Arkonor
    11396, // Mercoxit
];

/// Curated item names to price across hubs. Grouped by intent in source order.
pub const CURATED_TRADE_ITEMS: &[&str] = &[
    // Tech II drones (high value density, always in demand).
    "Hobgoblin II",
    "Hammerhead II",
    "Ogre II",
    "Warrior II",
    "Valkyrie II",
    "Berserker II",
    "Acolyte II",
    "Infiltrator II",
    "Praetor II",
    "Vespa II",
    "Wasp II",
    "Hornet II",
    // Tech II tackle / propulsion (fast movers).
    "Warp Scrambler II",
    "Warp Disruptor II",
    "Stasis Webifier II",
    "1MN Afterburner II",
    "10MN Afterburner II",
    "5MN Microwarpdrive II",
    "50MN Microwarpdrive II",
    "500MN Microwarpdrive II",
    // Tech II tank modules.
    "Damage Control II",
    "Assault Damage Control II",
    "Medium Shield Extender II",
    "Large Shield Extender II",
    "Adaptive Invulnerability Field II",
    "Multispectrum Shield Hardener II",
    "800mm Steel Plates II",
    "1600mm Steel Plates II",
    "Medium Armor Repairer II",
    "Large Armor Repairer II",
    "Multispectrum Energized Membrane II",
    "Reactive Armor Hardener",
    // Tech II gunnery / missiles (common doctrine fits).
    "200mm AutoCannon II",
    "425mm AutoCannon II",
    "220mm Vulcan AutoCannon II",
    "150mm Railgun II",
    "250mm Railgun II",
    "Heavy Assault Missile Launcher II",
    "Rapid Light Missile Launcher II",
    "Heavy Missile Launcher II",
    "Torpedo Launcher II",
    // Tech II EWAR / links / utility.
    "Sensor Booster II",
    "Tracking Computer II",
    "Signal Amplifier II",
    "Co-Processor II",
    "Nanofiber Internal Structure II",
    "Inertial Stabilizers II",
    "Expanded Cargohold II",
    "Cap Recharger II",
    "Capacitor Power Relay II",
    // Consumables / charges (very liquid).
    "Nanite Repair Paste",
    "Mobile Depot",
    "Mobile Tractor Unit",
    // Capital modules (high absolute spread between hubs).
    "Capital Shield Booster II",
    "Capital Armor Repairer II",
    "Capital Cap Battery II",
    "Capital Shield Extender II",
    "Capital Energy Neutralizer II",
    "Capital Remote Shield Booster II",
    "Capital Remote Armor Repairer II",
    "Capital Ancillary Current Router II",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn universe_is_nonempty_and_deduped() {
        assert!(CURATED_TRADE_ITEMS.len() > 40);
        let mut seen = std::collections::HashSet::new();
        for name in CURATED_TRADE_ITEMS {
            assert!(seen.insert(*name), "duplicate item in universe: {name}");
        }
    }
}
