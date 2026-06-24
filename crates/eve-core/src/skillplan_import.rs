//! Import skill plans from external tools.
//!
//! Supports two common shapes (no XML dependency — a light, tolerant scan):
//! - **EVEMon** plan exports: `<entry skill="Caldari Cruiser" level="4" .../>`.
//! - **Plain text**: one skill per line, level as a Roman numeral or digit, e.g.
//!   `Caldari Cruiser V`, `Hull Upgrades 4`, `Shield Management - III`.
//!
//! The parser is pure and returns `(skill_name, level)` pairs; name→type_id
//! resolution and costing happen in the command layer against the SDE. Duplicate
//! skills keep the highest level requested.

/// One imported target: a skill name and the level to train to (1–5).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedSkill {
    pub name: String,
    pub level: i64,
}

/// Parse a Roman numeral (I–V) or a plain digit (1–5) into a level. Returns
/// `None` for anything outside 1–5.
pub fn parse_level(token: &str) -> Option<i64> {
    let t = token.trim().trim_matches(|c: char| !c.is_alphanumeric());
    let level = match t.to_ascii_uppercase().as_str() {
        "I" | "1" => 1,
        "II" | "2" => 2,
        "III" | "3" => 3,
        "IV" | "4" => 4,
        "V" | "5" => 5,
        _ => return None,
    };
    Some(level)
}

/// Extract the value of an attribute like `skill="..."` from an EVEMon entry
/// line. Pure, tolerant of single or double quotes.
fn xml_attr<'a>(line: &'a str, attr: &str) -> Option<&'a str> {
    let key = format!("{attr}=");
    let start = line.find(&key)? + key.len();
    let rest = &line[start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let body = &rest[1..];
    let end = body.find(quote)?;
    Some(&body[..end])
}

/// Parse one EVEMon `<entry .../>` element, if the line is one.
fn parse_evemon_entry(line: &str) -> Option<ImportedSkill> {
    if !line.contains("<entry") {
        return None;
    }
    let name = xml_attr(line, "skill")?.trim();
    let level = xml_attr(line, "level").and_then(parse_level)?;
    if name.is_empty() {
        return None;
    }
    Some(ImportedSkill { name: name.to_string(), level })
}

/// Parse one plain-text line: "<skill name> <level>", where level is the last
/// token (Roman or digit), optionally separated by " - ".
fn parse_text_line(line: &str) -> Option<ImportedSkill> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('<') || line.starts_with('#') {
        return None;
    }
    // Split off a trailing " - " separator if present.
    let core = line.rsplit_once(" - ").map(|(a, b)| format!("{a} {b}")).unwrap_or_else(|| line.to_string());
    let (name, level_tok) = core.rsplit_once(char::is_whitespace)?;
    let level = parse_level(level_tok)?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some(ImportedSkill { name: name.to_string(), level })
}

/// Parse a whole plan blob (EVEMon XML or plain text) into deduplicated targets,
/// keeping the highest level per skill. Pure.
pub fn parse_plan(text: &str) -> Vec<ImportedSkill> {
    let mut out: Vec<ImportedSkill> = Vec::new();
    for line in text.lines() {
        let parsed = parse_evemon_entry(line).or_else(|| parse_text_line(line));
        if let Some(skill) = parsed {
            match out.iter_mut().find(|s| s.name.eq_ignore_ascii_case(&skill.name)) {
                Some(existing) => existing.level = existing.level.max(skill.level),
                None => out.push(skill),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_roman_and_digits() {
        assert_eq!(parse_level("IV"), Some(4));
        assert_eq!(parse_level("v"), Some(5));
        assert_eq!(parse_level("3"), Some(3));
        assert_eq!(parse_level("VI"), None);
        assert_eq!(parse_level("foo"), None);
    }

    #[test]
    fn parses_evemon_xml() {
        let xml = r#"
            <plan>
              <entry skill="Caldari Cruiser" level="4" priority="1" />
              <entry skill="Shield Management" level='5' />
            </plan>
        "#;
        let p = parse_plan(xml);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], ImportedSkill { name: "Caldari Cruiser".into(), level: 4 });
        assert_eq!(p[1], ImportedSkill { name: "Shield Management".into(), level: 5 });
    }

    #[test]
    fn parses_plain_text_and_dedups_to_highest() {
        let text = "Hull Upgrades IV\nCaldari Cruiser - V\nHull Upgrades 5\n# comment\n";
        let p = parse_plan(text);
        assert_eq!(p.len(), 2);
        // Hull Upgrades appears twice (IV then 5) → keeps 5.
        let hull = p.iter().find(|s| s.name == "Hull Upgrades").unwrap();
        assert_eq!(hull.level, 5);
        let cruiser = p.iter().find(|s| s.name == "Caldari Cruiser").unwrap();
        assert_eq!(cruiser.level, 5);
    }

    #[test]
    fn ignores_garbage_lines() {
        assert!(parse_plan("just some prose with no level\n\n").is_empty());
    }
}
