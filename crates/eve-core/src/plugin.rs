//! Plugin / extension API (Phase 7): community-authored **read-only** dashboard
//! panels, defined declaratively.
//!
//! The EULA + trust line forbids letting a plugin run arbitrary native code or
//! take in-game/ESI actions. So a plugin is **data, not code**: a JSON manifest
//! whose panels each name one tool from EVE Commander's existing read-only tool
//! registry (the same surface the AI assistant drives) plus arguments. The host
//! runs the named tool and renders its result — a plugin can never do anything
//! the core read-only tools don't already allow, and [`validate`] rejects any
//! panel that references a tool outside the allowlist.
//!
//! Parsing + validation are pure and unit-tested; loading manifests from disk
//! and executing the tools lives in the shell.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// One panel a plugin contributes: a title plus a whitelisted tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginPanel {
    pub title: String,
    /// A tool name from the host's read-only registry.
    pub command: String,
    /// Arguments passed to the tool (a JSON object). Defaults to `{}`.
    #[serde(default)]
    pub args: serde_json::Value,
}

/// A plugin manifest (one `*.json` file in the plugins directory).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub panels: Vec<PluginPanel>,
}

/// Parse a manifest from JSON text. Pure.
pub fn parse_manifest(json: &str) -> Result<PluginManifest> {
    serde_json::from_str(json).map_err(|e| Error::other(format!("invalid plugin manifest: {e}")))
}

/// Validate a manifest against the host's allowlist of read-only tool names.
/// Returns a list of human-readable problems (empty = the plugin is safe to
/// run). Pure — the security gate every panel passes before it can execute.
pub fn validate(manifest: &PluginManifest, allowed_tools: &[&str]) -> Vec<String> {
    let mut errors = Vec::new();
    if manifest.id.trim().is_empty() {
        errors.push("missing plugin id".to_string());
    }
    if manifest.name.trim().is_empty() {
        errors.push("missing plugin name".to_string());
    }
    if manifest.panels.is_empty() {
        errors.push("plugin defines no panels".to_string());
    }
    for (i, panel) in manifest.panels.iter().enumerate() {
        if !allowed_tools.contains(&panel.command.as_str()) {
            errors.push(format!(
                "panel {} (\"{}\") uses a command not in the read-only registry: {}",
                i, panel.title, panel.command
            ));
        }
    }
    errors
}

/// The args of a panel as a JSON string for the tool executor (objects only;
/// anything else becomes `{}` so a malformed manifest can't pass odd input).
pub fn panel_args_json(panel: &PluginPanel) -> String {
    if panel.args.is_object() {
        panel.args.to_string()
    } else {
        "{}".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
        "id": "wallet-watch",
        "name": "Wallet Watcher",
        "version": "1.0.0",
        "author": "Capsuleer",
        "panels": [
            { "title": "My net worth", "command": "get_account_overview", "args": {} },
            { "title": "System risk", "command": "get_system_risk", "args": { "system": "Jita" } }
        ]
    }"#;

    #[test]
    fn parses_and_validates_against_allowlist() {
        let m = parse_manifest(MANIFEST).unwrap();
        assert_eq!(m.id, "wallet-watch");
        assert_eq!(m.panels.len(), 2);
        let allow = ["get_account_overview", "get_system_risk"];
        assert!(validate(&m, &allow).is_empty());
        // Args serialize back to a JSON object string for the executor.
        assert_eq!(panel_args_json(&m.panels[0]), "{}");
        assert!(panel_args_json(&m.panels[1]).contains("Jita"));
    }

    #[test]
    fn rejects_command_outside_registry() {
        let m = parse_manifest(MANIFEST).unwrap();
        // Only one of the two commands is allowed → the other is flagged.
        let errors = validate(&m, &["get_account_overview"]);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("get_system_risk"));
    }

    #[test]
    fn flags_empty_or_malformed_manifest() {
        let m = PluginManifest {
            id: String::new(),
            name: String::new(),
            version: String::new(),
            description: String::new(),
            author: String::new(),
            panels: Vec::new(),
        };
        let errors = validate(&m, &[]);
        assert!(errors.iter().any(|e| e.contains("id")));
        assert!(errors.iter().any(|e| e.contains("name")));
        assert!(errors.iter().any(|e| e.contains("no panels")));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse_manifest("not json").is_err());
    }
}
