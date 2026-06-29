//! Dogma stats: the deterministic fitting math (the PYFA-class core).
//!
//! Per the plan's "Calculation trust" gate, the numbers a player sees must match
//! the game. These functions are pure and validated against known EVE formulae
//! and golden values:
//! - **Stacking penalties** — the canonical `exp(-(i/2.67)^2)` falloff applied to
//!   the i-th-strongest module of a stacking group.
//! - **EHP** — buffer hit points scaled by resonance against a damage profile.
//! - **DPS / volley** — weapon damage × multiplier ÷ rate of fire.
//! - **Capacitor** — peak passive recharge and stability against a load.
//!
//! Applying individual module *effects* (which attribute each module changes) is
//! the dogma-effect layer above this; these are the equations it feeds.

use serde::{Deserialize, Serialize};

/// Stacking-penalty multiplier for the `i`-th module in a stacking group
/// (0 = strongest, unpenalized). Canonical EVE formula `exp(-(i/2.67)^2)`. Pure.
pub fn stacking_multiplier(i: usize) -> f64 {
    let x = i as f64 / 2.67;
    (-(x * x)).exp()
}

/// Apply a set of multiplicative bonuses with stacking penalties, returning the
/// combined multiplier. Each `bonus` is a fractional change (e.g. `0.30` for a
/// +30% module, `-0.25` for a −25% resistance module). Bonuses are sorted by
/// strength so the largest is unpenalized, matching the game. Pure.
pub fn stacked(bonuses: &[f64]) -> f64 {
    // Strongest-effect first (largest absolute change).
    let mut sorted: Vec<f64> = bonuses.to_vec();
    sorted.sort_by(|a, b| b.abs().partial_cmp(&a.abs()).unwrap_or(std::cmp::Ordering::Equal));
    let mut total = 1.0;
    for (i, b) in sorted.iter().enumerate() {
        total *= 1.0 + b * stacking_multiplier(i);
    }
    total
}

/// The four EVE damage types as a profile (fractions that should sum to 1.0 for
/// an "average" reference; the math works for any weights).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DamageProfile {
    pub em: f64,
    pub thermal: f64,
    pub kinetic: f64,
    pub explosive: f64,
}

impl DamageProfile {
    /// The EVE-wide "omni" reference: equal weight to all four damage types.
    pub fn uniform() -> Self {
        Self { em: 0.25, thermal: 0.25, kinetic: 0.25, explosive: 0.25 }
    }
}

/// One buffer layer (shield, armor, or hull): raw hit points and the *resonance*
/// per damage type (resonance = 1 − resist; 1.0 = no resist, 0.0 = immune).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub hp: f64,
    pub em: f64,
    pub thermal: f64,
    pub kinetic: f64,
    pub explosive: f64,
}

impl Layer {
    /// Effective HP of this layer against a damage profile: hp divided by the
    /// profile-weighted average resonance. Pure.
    pub fn ehp(&self, profile: &DamageProfile) -> f64 {
        let w = profile.em + profile.thermal + profile.kinetic + profile.explosive;
        if w <= 0.0 || self.hp <= 0.0 {
            return self.hp.max(0.0);
        }
        let avg_resonance = (profile.em * self.em
            + profile.thermal * self.thermal
            + profile.kinetic * self.kinetic
            + profile.explosive * self.explosive)
            / w;
        if avg_resonance <= 0.0 {
            return f64::INFINITY;
        }
        self.hp / avg_resonance
    }
}

/// Total effective HP across shield + armor + hull against a damage profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ehp {
    pub shield: f64,
    pub armor: f64,
    pub hull: f64,
    pub total: f64,
}

/// Sum the three buffer layers into a total EHP figure. Pure.
pub fn total_ehp(shield: &Layer, armor: &Layer, hull: &Layer, profile: &DamageProfile) -> Ehp {
    let s = shield.ehp(profile);
    let a = armor.ehp(profile);
    let h = hull.ehp(profile);
    Ehp { shield: s, armor: a, hull: h, total: s + a + h }
}

/// One damage source in a fit (a turret/launcher with its loaded charge, or a
/// flight of drones already summed). Damage is per shot/cycle; `rof_seconds` is
/// the time between cycles; `multiplier` is the hull/turret damage multiplier.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Weapon {
    pub em: f64,
    pub thermal: f64,
    pub kinetic: f64,
    pub explosive: f64,
    pub multiplier: f64,
    pub rof_seconds: f64,
}

impl Weapon {
    /// Damage delivered in a single cycle (after the multiplier). Pure.
    pub fn volley(&self) -> f64 {
        (self.em + self.thermal + self.kinetic + self.explosive) * self.multiplier
    }
    /// Sustained damage per second. Pure.
    pub fn dps(&self) -> f64 {
        if self.rof_seconds <= 0.0 {
            return 0.0;
        }
        self.volley() / self.rof_seconds
    }
}

/// Aggregate DPS + alpha (volley) across a fit's weapons. Pure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageStats {
    pub dps: f64,
    pub volley: f64,
}

/// Sum DPS and volley across all weapons in a fit. Pure.
pub fn fit_damage(weapons: &[Weapon]) -> DamageStats {
    DamageStats {
        dps: weapons.iter().map(Weapon::dps).sum(),
        volley: weapons.iter().map(Weapon::volley).sum(),
    }
}

/// Capacitor stability against a steady load. `capacity` GJ, `recharge_seconds`
/// is the cap recharge time constant, `load` is GJ/s consumed by active modules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapStats {
    /// Peak passive recharge (GJ/s), which occurs at ~25% capacitor.
    pub peak_recharge: f64,
    /// Net GJ/s at peak recharge (peak_recharge − load).
    pub net_at_peak: f64,
    pub stable: bool,
    /// When unstable, rough seconds to empty from full (simple linear estimate).
    pub seconds_to_empty: Option<f64>,
}

/// Capacitor stability check. Peak recharge uses the standard
/// `2.5 × capacity / recharge_time`. A fit is stable when the load never exceeds
/// peak recharge. Pure.
pub fn cap_stats(capacity: f64, recharge_seconds: f64, load: f64) -> CapStats {
    let peak_recharge = if recharge_seconds > 0.0 { 2.5 * capacity / recharge_seconds } else { 0.0 };
    let net_at_peak = peak_recharge - load;
    let stable = net_at_peak >= 0.0;
    // Unstable: approximate drain time as capacity / (load − avg recharge).
    // Use half of peak as a rough average passive contribution while draining.
    let seconds_to_empty = if stable {
        None
    } else {
        let drain = (load - peak_recharge * 0.5).max(1e-6);
        Some(capacity / drain)
    };
    CapStats { peak_recharge, net_at_peak, stable, seconds_to_empty }
}

// ---- attribute-map builders (fed by `Sde::type_attributes`) ----------------
//
// Dogma attribute ids, named for the builders below. Resonance attributes are
// stored as multipliers (1.0 = no resist), so they map straight onto `Layer`.
pub mod attr {
    pub const HULL_HP: i64 = 9;
    pub const SHIELD_HP: i64 = 263;
    pub const ARMOR_HP: i64 = 265;
    // Hull resonances em/th/kin/exp.
    pub const HULL_EM: i64 = 113;
    pub const HULL_TH: i64 = 110;
    pub const HULL_KIN: i64 = 109;
    pub const HULL_EXP: i64 = 111;
    // Shield resonances.
    pub const SH_EM: i64 = 271;
    pub const SH_TH: i64 = 274;
    pub const SH_KIN: i64 = 273;
    pub const SH_EXP: i64 = 272;
    // Armor resonances.
    pub const AR_EM: i64 = 267;
    pub const AR_TH: i64 = 270;
    pub const AR_KIN: i64 = 269;
    pub const AR_EXP: i64 = 268;
    pub const CAP_CAPACITY: i64 = 482;
    pub const CAP_RECHARGE_MS: i64 = 55;
    // Damage (charges / drones) em/th/kin/exp.
    pub const DMG_EM: i64 = 114;
    pub const DMG_TH: i64 = 118;
    pub const DMG_KIN: i64 = 117;
    pub const DMG_EXP: i64 = 116;
    pub const DAMAGE_MULTIPLIER: i64 = 64;
    pub const ROF_MS: i64 = 51;
}

type Attrs = std::collections::HashMap<i64, f64>;

fn get(a: &Attrs, id: i64) -> f64 {
    a.get(&id).copied().unwrap_or(0.0)
}

/// Resonance defaults to 1.0 (no resist) when the SDE omits it, so a hull with
/// no stored resonance still yields its raw HP as EHP rather than dividing by 0.
fn resonance(a: &Attrs, id: i64) -> f64 {
    a.get(&id).copied().unwrap_or(1.0)
}

/// Build the three buffer layers (shield, armor, hull) from a ship's attribute
/// map. Module effects are not applied — this is the bare hull. Pure.
pub fn ship_layers(a: &Attrs) -> (Layer, Layer, Layer) {
    let shield = Layer {
        hp: get(a, attr::SHIELD_HP),
        em: resonance(a, attr::SH_EM),
        thermal: resonance(a, attr::SH_TH),
        kinetic: resonance(a, attr::SH_KIN),
        explosive: resonance(a, attr::SH_EXP),
    };
    let armor = Layer {
        hp: get(a, attr::ARMOR_HP),
        em: resonance(a, attr::AR_EM),
        thermal: resonance(a, attr::AR_TH),
        kinetic: resonance(a, attr::AR_KIN),
        explosive: resonance(a, attr::AR_EXP),
    };
    let hull = Layer {
        hp: get(a, attr::HULL_HP),
        em: resonance(a, attr::HULL_EM),
        thermal: resonance(a, attr::HULL_TH),
        kinetic: resonance(a, attr::HULL_KIN),
        explosive: resonance(a, attr::HULL_EXP),
    };
    (shield, armor, hull)
}

/// Capacitor capacity (GJ) and recharge time (seconds) from a ship's attributes.
/// Pure.
pub fn ship_cap(a: &Attrs) -> (f64, f64) {
    (get(a, attr::CAP_CAPACITY), get(a, attr::CAP_RECHARGE_MS) / 1000.0)
}

/// Build a [`Weapon`] from a damage source's attributes. For turrets, damage
/// comes from the loaded `charge` and is scaled by the turret's damage
/// multiplier; for missiles the charge carries the damage and the multiplier is
/// one. Rate of fire (attr 51) is in milliseconds. Returns `None` when there is
/// no damage or no cycle time. Pure.
pub fn weapon_from(module: &Attrs, charge: Option<&Attrs>) -> Option<Weapon> {
    // Damage is on the charge if one is loaded, else on the module itself
    // (drones, some NPC-style entities).
    let src = charge.unwrap_or(module);
    let (em, thermal, kinetic, explosive) = (
        get(src, attr::DMG_EM),
        get(src, attr::DMG_TH),
        get(src, attr::DMG_KIN),
        get(src, attr::DMG_EXP),
    );
    if em + thermal + kinetic + explosive <= 0.0 {
        return None;
    }
    // Multiplier and rof come from the module (turret/launcher/drone).
    let multiplier = module.get(&attr::DAMAGE_MULTIPLIER).copied().unwrap_or(1.0);
    let rof_seconds = get(module, attr::ROF_MS) / 1000.0;
    if rof_seconds <= 0.0 {
        return None;
    }
    Some(Weapon { em, thermal, kinetic, explosive, multiplier, rof_seconds })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(i64, f64)]) -> Attrs {
        pairs.iter().copied().collect()
    }

    #[test]
    fn ship_layers_and_cap_from_attributes() {
        let a = map(&[
            (attr::SHIELD_HP, 2000.0),
            (attr::ARMOR_HP, 1500.0),
            (attr::HULL_HP, 1000.0),
            (attr::SH_EM, 0.0), // 100% EM resist on shield
            (attr::CAP_CAPACITY, 1000.0),
            (attr::CAP_RECHARGE_MS, 200_000.0),
        ]);
        let (shield, armor, hull) = ship_layers(&a);
        assert_eq!(shield.hp, 2000.0);
        assert_eq!(shield.em, 0.0); // stored resonance honored
        assert_eq!(armor.em, 1.0); // missing → no resist
        assert_eq!(hull.hp, 1000.0);
        let (cap, recharge) = ship_cap(&a);
        assert_eq!(cap, 1000.0);
        assert_eq!(recharge, 200.0);
    }

    #[test]
    fn weapon_from_charge_scaled_by_turret_multiplier() {
        let turret = map(&[(attr::DAMAGE_MULTIPLIER, 2.0), (attr::ROF_MS, 4000.0)]);
        let charge = map(&[(attr::DMG_EM, 10.0), (attr::DMG_TH, 10.0)]);
        let w = weapon_from(&turret, Some(&charge)).unwrap();
        // (10+10) * 2 = 40 volley, over 4s → 10 dps.
        assert!((w.volley() - 40.0).abs() < 1e-6);
        assert!((w.dps() - 10.0).abs() < 1e-6);
        // No charge + no module damage → None.
        assert!(weapon_from(&turret, None).is_none());
    }

    #[test]
    fn stacking_first_unpenalized_then_falls_off() {
        assert!((stacking_multiplier(0) - 1.0).abs() < 1e-9);
        // exp(-(1/2.67)^2) ≈ 0.8691.
        assert!((stacking_multiplier(1) - 0.8691).abs() < 1e-3);
        assert!(stacking_multiplier(2) < stacking_multiplier(1));
    }

    #[test]
    fn stacked_bonuses_apply_penalty_to_weaker() {
        // Two +30% mods: 1 * (1+0.3) * (1+0.3*0.8691) = 1.3 * 1.2607 = 1.639.
        let m = stacked(&[0.30, 0.30]);
        assert!((m - 1.6389).abs() < 1e-3);
        // A single bonus is unpenalized.
        assert!((stacked(&[0.30]) - 1.30).abs() < 1e-9);
    }

    #[test]
    fn ehp_no_resist_is_raw_hp_with_resist_doubles() {
        let none = Layer { hp: 1000.0, em: 1.0, thermal: 1.0, kinetic: 1.0, explosive: 1.0 };
        assert!((none.ehp(&DamageProfile::uniform()) - 1000.0).abs() < 1e-6);
        // Uniform 50% resist (resonance 0.5) → EHP doubles.
        let half = Layer { hp: 1000.0, em: 0.5, thermal: 0.5, kinetic: 0.5, explosive: 0.5 };
        assert!((half.ehp(&DamageProfile::uniform()) - 2000.0).abs() < 1e-6);
    }

    #[test]
    fn total_ehp_sums_layers() {
        let l = |hp: f64| Layer { hp, em: 1.0, thermal: 1.0, kinetic: 1.0, explosive: 1.0 };
        let e = total_ehp(&l(1000.0), &l(2000.0), &l(500.0), &DamageProfile::uniform());
        assert!((e.total - 3500.0).abs() < 1e-6);
    }

    #[test]
    fn dps_and_volley() {
        // 100 total damage/shot, x2 multiplier, 2s rof → volley 200, dps 100.
        let w = Weapon { em: 25.0, thermal: 25.0, kinetic: 25.0, explosive: 25.0, multiplier: 2.0, rof_seconds: 2.0 };
        assert!((w.volley() - 200.0).abs() < 1e-6);
        assert!((w.dps() - 100.0).abs() < 1e-6);
        let agg = fit_damage(&[w, w]);
        assert!((agg.dps - 200.0).abs() < 1e-6);
        assert!((agg.volley - 400.0).abs() < 1e-6);
    }

    #[test]
    fn cap_stable_when_load_below_peak() {
        // 1000 GJ, 200s recharge → peak 12.5 GJ/s.
        let s = cap_stats(1000.0, 200.0, 10.0);
        assert!((s.peak_recharge - 12.5).abs() < 1e-6);
        assert!(s.stable);
        assert!(s.seconds_to_empty.is_none());

        let u = cap_stats(1000.0, 200.0, 20.0);
        assert!(!u.stable);
        assert!(u.seconds_to_empty.unwrap() > 0.0);
    }
}
