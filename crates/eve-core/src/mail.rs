//! EVEmail: mail headers and reading a single mail.
//!
//! `GET /characters/{id}/mail/` returns the most recent ~50 headers (cursor-
//! paginated via `last_mail_id`; we fetch the latest page for now), and
//! `GET /characters/{id}/mail/{mail_id}/` returns one mail's body. EVE mail
//! bodies carry the game's HTML-ish markup, so [`strip_markup`] reduces them to
//! readable plain text. That and [`unread_count`] are pure and unit-tested.

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// A mail recipient reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailRecipient {
    pub recipient_id: i64,
    #[serde(default)]
    pub recipient_type: String,
}

/// A mail header (ESI `GET /characters/{id}/mail/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailHeader {
    pub mail_id: i64,
    #[serde(default)]
    pub subject: String,
    /// Sender id (character/corporation/alliance/mailing-list).
    #[serde(default)]
    pub from: i64,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub is_read: bool,
    #[serde(default)]
    pub labels: Vec<i64>,
    #[serde(default)]
    pub recipients: Vec<MailRecipient>,
}

/// A single mail with its body (ESI `GET /characters/{id}/mail/{mail_id}/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mail {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub from: i64,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub labels: Vec<i64>,
    #[serde(default)]
    pub recipients: Vec<MailRecipient>,
}

/// Number of unread headers.
pub fn unread_count(headers: &[MailHeader]) -> usize {
    headers.iter().filter(|h| !h.is_read).count()
}

/// Reduce EVE's HTML-ish mail markup to readable plain text: `<br>` becomes a
/// newline, all other tags are dropped, and the common entities are decoded.
pub fn strip_markup(markup: &str) -> String {
    let mut out = String::new();
    let mut chars = markup.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            // Consume up to the closing '>'.
            let mut tag = String::new();
            for t in chars.by_ref() {
                if t == '>' {
                    break;
                }
                tag.push(t);
            }
            // <br> / <br/> become a line break; every other tag is dropped.
            if tag.trim_start().to_ascii_lowercase().starts_with("br") {
                out.push('\n');
            }
        } else {
            out.push(c);
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// Typed, authenticated mail reads over the cache-first ESI client.
#[derive(Clone)]
pub struct MailClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl MailClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The latest page of mail headers (most recent first per ESI).
    pub async fn headers(&self, character_id: i64) -> Result<Vec<MailHeader>> {
        let ep = endpoint("mail").ok_or_else(|| Error::other("unknown endpoint 'mail'"))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json::<Vec<MailHeader>>(&ep.path_for(character_id), &token)
            .await
    }

    /// Read a single mail's body.
    pub async fn body(&self, character_id: i64, mail_id: i64) -> Result<Mail> {
        let path = format!("/latest/characters/{character_id}/mail/{mail_id}/");
        let token = self.tokens.access_token(character_id).await?;
        self.esi.get_auth_json::<Mail>(&path, &token).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_mail_header() {
        let json = r#"{
            "from": 90000001,
            "is_read": false,
            "labels": [4],
            "mail_id": 12345,
            "recipients": [{"recipient_id": 90000002, "recipient_type": "character"}],
            "subject": "Op tonight",
            "timestamp": "2026-06-20T10:00:00Z"
        }"#;
        let h: MailHeader = serde_json::from_str(json).unwrap();
        assert_eq!(h.mail_id, 12345);
        assert_eq!(h.subject, "Op tonight");
        assert!(!h.is_read);
        assert_eq!(h.recipients[0].recipient_id, 90000002);
    }

    #[test]
    fn deserializes_mail_body() {
        let json = r#"{
            "subject": "Hi",
            "from": 90000001,
            "body": "<font size=\"12\">Line one<br>Line two</font>",
            "timestamp": "2026-06-20T10:00:00Z",
            "read": true,
            "labels": [],
            "recipients": []
        }"#;
        let m: Mail = serde_json::from_str(json).unwrap();
        assert_eq!(m.subject, "Hi");
        assert!(m.read);
        assert!(m.body.contains("Line one"));
    }

    #[test]
    fn counts_unread() {
        let headers = vec![
            MailHeader { mail_id: 1, subject: "a".into(), from: 0, timestamp: String::new(), is_read: false, labels: vec![], recipients: vec![] },
            MailHeader { mail_id: 2, subject: "b".into(), from: 0, timestamp: String::new(), is_read: true, labels: vec![], recipients: vec![] },
            MailHeader { mail_id: 3, subject: "c".into(), from: 0, timestamp: String::new(), is_read: false, labels: vec![], recipients: vec![] },
        ];
        assert_eq!(unread_count(&headers), 2);
    }

    #[test]
    fn strips_markup_to_plain_text() {
        let body = "<font size=\"14\" color=\"#bfffffff\">Hello o7<br><br>See you at <a href=\"showinfo:5//30000142\">Jita</a>.</font>";
        let text = strip_markup(body);
        assert_eq!(text, "Hello o7\n\nSee you at Jita.");
    }

    #[test]
    fn strips_decodes_entities() {
        assert_eq!(strip_markup("a &amp; b &lt;tag&gt;"), "a & b <tag>");
    }
}
