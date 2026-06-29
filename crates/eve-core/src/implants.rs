//! Implant classification for the RPG-style implant fitter.
//!
//! Implants occupy slots 1–10 (dogma attribute 331). Slots 1–5 are the attribute
//! enhancers (the five canonical series below); slots 6–10 are skill hardwirings.
//! The boost category is derived from the implant's series name — robust for the
//! attribute enhancers, with hardwirings grouped generically. Pure.

/// A coarse boost category used for filtering in the fitter UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoostCategory {
    Intelligence,
    Memory,
    Perception,
    Willpower,
    Charisma,
    Hardwiring,
}

impl BoostCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            BoostCategory::Intelligence => "Intelligence",
            BoostCategory::Memory => "Memory",
            BoostCategory::Perception => "Perception",
            BoostCategory::Willpower => "Willpower",
            BoostCategory::Charisma => "Charisma",
            BoostCategory::Hardwiring => "Hardwiring",
        }
    }
}

/// Classify an implant by its name. The five attribute-enhancer series are
/// matched by their canonical names; everything else (slot 6–10 hardwirings) is
/// `Hardwiring`. Pure.
pub fn classify(name: &str) -> BoostCategory {
    let n = name.to_ascii_lowercase();
    if n.contains("cybernetic subprocessor") {
        BoostCategory::Intelligence
    } else if n.contains("memory augmentation") {
        BoostCategory::Memory
    } else if n.contains("ocular filter") {
        BoostCategory::Perception
    } else if n.contains("neural boost") {
        BoostCategory::Willpower
    } else if n.contains("social adaptation chip") {
        BoostCategory::Charisma
    } else {
        BoostCategory::Hardwiring
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_enhancers_classified() {
        assert_eq!(classify("Cybernetic Subprocessor - Standard"), BoostCategory::Intelligence);
        assert_eq!(classify("Memory Augmentation - Improved"), BoostCategory::Memory);
        assert_eq!(classify("Ocular Filter - Basic"), BoostCategory::Perception);
        assert_eq!(classify("Neural Boost - Advanced"), BoostCategory::Willpower);
        assert_eq!(classify("Social Adaptation Chip - Elite"), BoostCategory::Charisma);
    }

    #[test]
    fn hardwirings_default() {
        assert_eq!(classify("Zainou 'Gnome' Shield Management SM-703"), BoostCategory::Hardwiring);
        assert_eq!(classify("Inherent Implants 'Squire' Capacitor"), BoostCategory::Hardwiring);
    }
}
