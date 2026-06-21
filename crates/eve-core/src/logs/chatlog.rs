//! EVE Chatlog parsing → Local intel (who's talking + current system).
//!
//! Chat lines look like `[ 2024.01.15 12:35:10 ] Some Pilot > message`. The
//! system messenger "EVE System" announces the system on entry
//! (`Channel changed to Local : Jita`). EVE has no "who is in Local" API and
//! reading game memory is bannable, so the honest, EULA-safe signal is *who
//! speaks*: a pilot who talks in Local is present right now. Parsing is pure +
//! unit-tested.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::split_timestamp;

/// The system-messenger sender name (not a real pilot).
const EVE_SYSTEM: &str = "EVE System";

/// One parsed chat line. (Internal — only [`LocalIntel`] crosses IPC.)
#[derive(Debug, Clone, PartialEq)]
pub struct ChatLine {
    pub at: OffsetDateTime,
    pub sender: String,
    pub message: String,
}

/// Parse one chat line `[ ts ] Sender > message`. Pure.
pub fn parse_chat_line(line: &str) -> Option<ChatLine> {
    let (at, rest) = split_timestamp(line)?;
    let (sender, message) = rest.split_once('>')?;
    let sender = sender.trim();
    if sender.is_empty() {
        return None;
    }
    Some(ChatLine {
        at,
        sender: sender.to_string(),
        message: message.trim().to_string(),
    })
}

/// Local-channel intel derived from a chatlog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalIntel {
    /// Current system, from the latest "Channel changed to Local : X" line.
    pub system: Option<String>,
    /// Distinct pilots who spoke, most recent first (excludes "EVE System").
    pub speakers: Vec<String>,
    pub line_count: usize,
}

/// Reduce a Local chatlog to its current system + the pilots who have spoken
/// (most recent first). Pure.
pub fn summarize_local(text: &str) -> LocalIntel {
    let mut system = None;
    let mut speakers: Vec<String> = Vec::new();
    let mut line_count = 0;

    for line in text.lines() {
        let Some(parsed) = parse_chat_line(line) else { continue };
        line_count += 1;

        if parsed.sender == EVE_SYSTEM {
            if let Some((_, sys)) = parsed.message.rsplit_once(':') {
                let sys = sys.trim();
                if !sys.is_empty() {
                    system = Some(sys.to_string());
                }
            }
            continue;
        }
        // Keep most-recent-first, de-duplicated: drop an earlier mention and
        // push to the front.
        speakers.retain(|s| s != &parsed.sender);
        speakers.insert(0, parsed.sender);
    }

    LocalIntel { system, speakers, line_count }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
[ 2024.01.15 12:00:00 ] EVE System > Channel changed to Local : Jita
[ 2024.01.15 12:35:10 ] Bob Pilot > anyone selling tritanium?
[ 2024.01.15 12:35:20 ] Alice Hunter > o7
[ 2024.01.15 12:36:00 ] Bob Pilot > still here";

    #[test]
    fn parses_sender_and_message() {
        let c = parse_chat_line("[ 2024.01.15 12:35:10 ] Bob Pilot > hello there").unwrap();
        assert_eq!(c.sender, "Bob Pilot");
        assert_eq!(c.message, "hello there");
    }

    #[test]
    fn summary_extracts_system_and_recent_speakers() {
        let intel = summarize_local(LOG);
        assert_eq!(intel.system.as_deref(), Some("Jita"));
        // Bob spoke last → first; deduped to one entry.
        assert_eq!(intel.speakers, vec!["Bob Pilot", "Alice Hunter"]);
    }

    #[test]
    fn excludes_eve_system_from_speakers() {
        let intel = summarize_local(LOG);
        assert!(!intel.speakers.iter().any(|s| s == "EVE System"));
    }
}
