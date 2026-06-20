//! A small curated seed of common EVE types and systems.
//!
//! The full Static Data Export is large and produced by `sde-tools` from CCP's
//! export. Until that prebuilt `sde.sqlite` is present, this seed lets id→name
//! resolution still show real names for the most common cases — minerals, the
//! classic ores (so the mining ledger reads cleanly), a handful of iconic
//! ships, and the major trade hubs. Anything not in the seed (or the full SDE)
//! falls back to `Type {id}`.
//!
//! Entries are limited to long-stable, high-confidence ids on purpose: a wrong
//! name is worse than an honest `Type {id}`.

/// `(type_id, name)` for common items.
pub const SEED_TYPES: &[(i64, &str)] = &[
    // --- Minerals ---
    (34, "Tritanium"),
    (35, "Pyerite"),
    (36, "Mexallon"),
    (37, "Isogen"),
    (38, "Nocxium"),
    (39, "Zydrine"),
    (40, "Megacyte"),
    (11399, "Morphite"),
    // --- Ores (classic base variants) ---
    (1230, "Veldspar"),
    (1228, "Scordite"),
    (1224, "Pyroxeres"),
    (18, "Plagioclase"),
    (1227, "Omber"),
    (20, "Kernite"),
    (1226, "Jaspet"),
    (1231, "Hemorphite"),
    (21, "Hedbergite"),
    (1229, "Gneiss"),
    (1232, "Dark Ochre"),
    (19, "Spodumain"),
    (1225, "Crokite"),
    (1223, "Bistot"),
    (22, "Arkonor"),
    (11396, "Mercoxit"),
    // --- Iconic ships ---
    (670, "Capsule"),
    (587, "Rifter"),
    (597, "Punisher"),
    (603, "Merlin"),
    (593, "Tristan"),
    (16240, "Catalyst"),
    (24698, "Drake"),
    (24702, "Hurricane"),
    (638, "Raven"),
    (641, "Megathron"),
    (645, "Dominix"),
    (642, "Apocalypse"),
];

/// `(system_id, name, security)` for the major trade hubs.
pub const SEED_SYSTEMS: &[(i64, &str, f64)] = &[
    (30000142, "Jita", 0.95),
    (30002187, "Amarr", 1.0),
    (30002659, "Dodixie", 0.90),
    (30002510, "Rens", 0.89),
    (30002053, "Hek", 0.50),
];
