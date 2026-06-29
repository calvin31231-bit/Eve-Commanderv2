//! Implant classification for the RPG-style implant fitter.
//!
//! Implants occupy slots 1–10 (dogma attribute 331). Slots 1–5 are the attribute
//! enhancers (the five canonical series); slots 6–10 are skill hardwirings whose
//! name carries the skill they boost (e.g. "Zainou 'Gnome' Shield Management" →
//! Shield). The boost category is derived from those reliable name patterns. Pure.

/// Classify an implant into a boost category from its name. The five attribute
/// enhancers are matched by their canonical series; hardwirings are bucketed by
/// the skill keyword in their name; anything unmatched is `Hardwiring`. Pure.
pub fn classify(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();

    // Slot 1–5 attribute enhancers (exact series names).
    if n.contains("cybernetic subprocessor") {
        return "Intelligence";
    }
    if n.contains("memory augmentation") {
        return "Memory";
    }
    if n.contains("ocular filter") {
        return "Perception";
    }
    if n.contains("neural boost") {
        return "Willpower";
    }
    if n.contains("social adaptation chip") {
        return "Charisma";
    }

    // Slot 6–10 hardwirings: bucket by the skill keyword in the name. Order is
    // most-specific first so e.g. "target navigation" lands under Missiles.
    const KEYWORDS: &[(&str, &str)] = &[
        ("shield", "Shield"),
        ("armor", "Armor"),
        ("hull", "Armor"),
        ("capacitor", "Capacitor"),
        ("energy management", "Capacitor"),
        ("energy systems", "Capacitor"),
        ("drone", "Drones"),
        ("missile", "Missiles"),
        ("launcher", "Missiles"),
        ("warhead", "Missiles"),
        ("rapid launch", "Missiles"),
        ("target navigation", "Missiles"),
        ("guided missile", "Missiles"),
        ("surgical strike", "Gunnery"),
        ("sharpshooter", "Gunnery"),
        ("motion prediction", "Gunnery"),
        ("rapid firing", "Gunnery"),
        ("controlled bursts", "Gunnery"),
        ("trajectory analysis", "Gunnery"),
        ("gunnery", "Gunnery"),
        ("turret", "Gunnery"),
        ("navigation", "Navigation"),
        ("afterburner", "Navigation"),
        ("evasive maneuvering", "Navigation"),
        ("acceleration control", "Navigation"),
        ("warp drive", "Navigation"),
        ("fuel conservation", "Navigation"),
        ("power grid", "Engineering"),
        ("engineering", "Engineering"),
        ("thermodynamics", "Engineering"),
        ("electronics", "Targeting"),
        ("targeting", "Targeting"),
        ("long range targeting", "Targeting"),
        ("signature analysis", "Targeting"),
        ("sensor", "Targeting"),
        ("industry", "Industry"),
        ("mining", "Industry"),
        ("learning", "Learning"),
    ];
    for (kw, cat) in KEYWORDS {
        if n.contains(kw) {
            return cat;
        }
    }
    "Hardwiring"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_enhancers_classified() {
        assert_eq!(classify("Cybernetic Subprocessor - Standard"), "Intelligence");
        assert_eq!(classify("Memory Augmentation - Improved"), "Memory");
        assert_eq!(classify("Ocular Filter - Basic"), "Perception");
        assert_eq!(classify("Neural Boost - Advanced"), "Willpower");
        assert_eq!(classify("Social Adaptation Chip - Elite"), "Charisma");
    }

    #[test]
    fn hardwirings_bucketed_by_skill_keyword() {
        assert_eq!(classify("Zainou 'Gnome' Shield Management SM-703"), "Shield");
        assert_eq!(classify("Inherent Implants 'Squire' Capacitor Management EM-803"), "Capacitor");
        assert_eq!(classify("Eifyr and Co. 'Gunslinger' Surgical Strike SS-905"), "Gunnery");
        assert_eq!(classify("Zainou 'Deadeye' Guided Missile Precision GP-805"), "Missiles");
        assert_eq!(classify("Inherent Implants 'Lancer' Controlled Bursts CB-705"), "Gunnery");
        assert_eq!(classify("Eifyr and Co. 'Rogue' Acceleration Control AC-605"), "Navigation");
        assert_eq!(classify("Some Unknown Implant"), "Hardwiring");
    }
}
