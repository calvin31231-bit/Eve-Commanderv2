//! Core data model: characters, groups ("hats"), and scope sets.
//!
//! A character is first-class — it owns its tokens, granted scopes, and poll
//! schedule. Groups let the user define operational sets (an "industry stable",
//! a "scout fleet") that drive cross-character aggregations.

use serde::{Deserialize, Serialize};

/// An authenticated EVE character.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Character {
    /// ESI character id.
    pub id: i64,
    pub name: String,
    pub corporation_id: Option<i64>,
    pub alliance_id: Option<i64>,
    /// ESI scopes the user has granted for this character (incremental).
    pub scopes: Vec<String>,
    /// Whether this character is currently the "active"/foreground one, which
    /// the scheduler polls at full cadence.
    pub active: bool,
}

impl Character {
    pub fn new(id: i64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            corporation_id: None,
            alliance_id: None,
            scopes: Vec::new(),
            active: false,
        }
    }

    /// Whether the character has granted a specific scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

/// A user-defined set of characters ("hat") used for aggregated views.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CharacterGroup {
    pub id: i64,
    pub name: String,
    /// Member character ids.
    pub members: Vec<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_check() {
        let mut c = Character::new(90000001, "Test Pilot");
        assert!(!c.has_scope("esi-skills.read_skills.v1"));
        c.scopes.push("esi-skills.read_skills.v1".to_string());
        assert!(c.has_scope("esi-skills.read_skills.v1"));
    }
}
