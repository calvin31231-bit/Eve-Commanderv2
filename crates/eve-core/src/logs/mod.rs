//! Local EVE log reading — the desktop-only moat.
//!
//! EVE writes two kinds of logs under `~/Documents/EVE/logs/`:
//! - **Gamelogs/** — combat events → DPS / after-action ([`gamelog`]).
//! - **Chatlogs/** — channel chat, incl. Local → intel / who's talking
//!   ([`chatlog`]).
//!
//! These are passive reads of files the game writes — explicitly EULA-allowed
//! (no memory access, no automation). The line parsers are pure and unit-tested;
//! this module adds the I/O around them: locating the log dirs, decoding the
//! files (EVE writes UTF-16LE), and picking the most-recent log.

pub mod chatlog;
pub mod gamelog;

use std::path::{Path, PathBuf};

use time::format_description::BorrowedFormatItem;
use time::{macros::format_description, OffsetDateTime, PrimitiveDateTime};

const TS_FORMAT: &[BorrowedFormatItem<'_>] =
    format_description!("[year].[month].[day] [hour]:[minute]:[second]");

/// Parse the `[ 2024.01.15 12:34:57 ]` prefix common to all log lines, returning
/// the inner time (read as UTC — only deltas matter) and the rest of the line.
pub(crate) fn split_timestamp(line: &str) -> Option<(OffsetDateTime, &str)> {
    let line = line.trim_start();
    let rest = line.strip_prefix('[')?;
    let (ts, after) = rest.split_once(']')?;
    let dt = PrimitiveDateTime::parse(ts.trim(), &TS_FORMAT).ok()?;
    Some((dt.assume_utc(), after.trim_start()))
}

/// Decode raw log bytes to a string. EVE typically writes UTF-16LE (with BOM);
/// some logs are UTF-8. We sniff the BOM and fall back to a UTF-8 read.
pub fn decode_log_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    // Heuristic: lots of NUL bytes ⇒ UTF-16LE without a BOM.
    let nul = bytes.iter().filter(|&&b| b == 0).count();
    if !bytes.is_empty() && nul * 3 > bytes.len() {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Resolve the root EVE logs directory. `EVE_COMMANDER_LOG_DIR` overrides; else
/// `~/Documents/EVE/logs` via the platform home variable.
pub fn logs_root() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("EVE_COMMANDER_LOG_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(PathBuf::from(home).join("Documents").join("EVE").join("logs"))
}

/// The Gamelogs subdirectory, if the logs root resolves.
pub fn gamelogs_dir() -> Option<PathBuf> {
    logs_root().map(|r| r.join("Gamelogs"))
}

/// The Chatlogs subdirectory, if the logs root resolves.
pub fn chatlogs_dir() -> Option<PathBuf> {
    logs_root().map(|r| r.join("Chatlogs"))
}

/// The most recently modified `.txt` file in `dir` whose name starts with
/// `prefix` (empty = any), or `None` if the dir is missing/empty.
pub fn latest_log(dir: &Path, prefix: &str) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !prefix.is_empty() && !name.starts_with(prefix) {
            continue;
        }
        let mtime = entry.metadata().and_then(|m| m.modified()).ok()?;
        let is_newer = match &best {
            Some((t, _)) => mtime > *t,
            None => true,
        };
        if is_newer {
            best = Some((mtime, path));
        }
    }
    best.map(|(_, p)| p)
}

/// The `max` most-recently-modified `.txt` logs in `dir` matching `prefix`,
/// newest first. Used to pick up a multiboxer's separate per-character Gamelogs
/// from the current session for a combined fleet after-action report.
pub fn recent_logs(dir: &Path, prefix: &str, max: usize) -> Vec<PathBuf> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !prefix.is_empty() && !name.starts_with(prefix) {
            continue;
        }
        if let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) {
            found.push((mtime, path));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().take(max).map(|(_, p)| p).collect()
}

/// Read + decode a log file to a string.
pub fn read_log(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(decode_log_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf16le_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "Hi[]".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_log_bytes(&bytes), "Hi[]");
    }

    #[test]
    fn decodes_utf8() {
        assert_eq!(decode_log_bytes(b"plain ascii"), "plain ascii");
    }

    #[test]
    fn splits_timestamp() {
        let (_, rest) = split_timestamp("[ 2024.01.15 12:34:57 ] (combat) hi").unwrap();
        assert_eq!(rest, "(combat) hi");
        assert!(split_timestamp("no timestamp").is_none());
    }
}
