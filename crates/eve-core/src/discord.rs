//! Discord webhook dispatch for notifications.
//!
//! When a notification interrupts (meets the user's threshold), the shell can
//! also mirror it to a Discord channel via an incoming webhook — the
//! multi-channel notification path from the ROADMAP. The payload builder is pure
//! and unit-tested; the POST itself is a thin reqwest call. Off unless the user
//! configures a webhook URL.

use serde_json::json;

use crate::error::{Error, Result};
use crate::notify::{Notification, Severity};

/// Discord embed accent color for a severity (RGB int).
fn severity_color(severity: Severity) -> u32 {
    match severity {
        Severity::Info => 0x3B_82_F6,     // blue
        Severity::Warning => 0xF5_9E_0B,  // amber
        Severity::Critical => 0xEF_44_44, // red
    }
}

/// Build the Discord webhook JSON for a notification (a single embed). Pure.
pub fn webhook_payload(n: &Notification) -> serde_json::Value {
    json!({
        "embeds": [{
            "title": n.title,
            "description": n.body,
            "color": severity_color(n.severity),
            "footer": { "text": format!("EVE Commander · {}", n.category) },
        }]
    })
}

/// Posts notifications to a Discord incoming webhook.
#[derive(Clone)]
pub struct DiscordClient {
    http: reqwest::Client,
}

impl Default for DiscordClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscordClient {
    pub fn new() -> Self {
        Self { http: reqwest::Client::new() }
    }

    /// POST a notification to `webhook_url`. Errors (bad URL, network, Discord
    /// rate-limit) are returned for the caller to log; they never block the app.
    pub async fn send(&self, webhook_url: &str, n: &Notification) -> Result<()> {
        let resp = self
            .http
            .post(webhook_url)
            .json(&webhook_payload(n))
            .send()
            .await
            .map_err(|e| Error::other(format!("discord webhook: {e}")))?;
        if !resp.status().is_success() {
            return Err(Error::other(format!(
                "discord webhook returned {}",
                resp.status()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn note(sev: Severity) -> Notification {
        Notification::new("k", "Title", "Body text", sev, "fuel", SystemTime::now())
    }

    #[test]
    fn payload_has_one_embed_with_fields() {
        let p = webhook_payload(&note(Severity::Critical));
        let embed = &p["embeds"][0];
        assert_eq!(embed["title"], "Title");
        assert_eq!(embed["description"], "Body text");
        assert_eq!(embed["color"], 0xEF_44_44);
        assert!(embed["footer"]["text"].as_str().unwrap().contains("fuel"));
    }

    #[test]
    fn color_varies_by_severity() {
        assert_ne!(
            webhook_payload(&note(Severity::Info))["embeds"][0]["color"],
            webhook_payload(&note(Severity::Critical))["embeds"][0]["color"]
        );
    }
}
