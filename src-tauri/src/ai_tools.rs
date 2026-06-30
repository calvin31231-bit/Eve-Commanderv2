//! The AI tool registry: the curated, **read-only** slice of EVE Commander's
//! capabilities the orchestrator may call, plus an executor that runs a tool by
//! name against [`AppState`] and returns a JSON string for the model to narrate.
//!
//! EULA / guardrail note: every tool here is advisory/analytical and touches only
//! public or the player's own data. No state-changing or in-game action is
//! exposed to the model — those stay behind explicit user confirmation in the UI.

use eve_core::ai::ToolSpec;
use serde_json::{json, Value};

use crate::AppState;

/// The tool specs offered to the model. Kept small and read-only by design.
pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "search_item".into(),
            description: "Resolve an item name to its type_id via the SDE. Use this first when the \
                          user names an item, before pricing it."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": { "query": { "type": "string", "description": "Item name or prefix" } },
                "required": ["query"]
            }),
        },
        ToolSpec {
            name: "compare_hubs".into(),
            description: "Best buy/sell price for an item type across the five major trade hubs \
                          (Jita, Amarr, Dodixie, Rens, Hek)."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": { "type_id": { "type": "integer" } },
                "required": ["type_id"]
            }),
        },
        ToolSpec {
            name: "scan_arbitrage".into(),
            description: "Best cross-hub buy-low/sell-high hauls on a curated set of liquid items, \
                          ranked by per-unit profit after sales tax."
                .into(),
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "system_risk".into(),
            description: "Unified 0-100 threat score for the active character's current system, \
                          fusing recent kills, security and a gate-camp signal."
                .into(),
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "fit_stats".into(),
            description: "Dogma stats for an EFT fit: EHP (with buffer/resist modules), DPS, \
                          volley, and capacitor. Use to critique or compare fits."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": { "eft": { "type": "string", "description": "EFT fit text" } },
                "required": ["eft"]
            }),
        },
        ToolSpec {
            name: "account_overview".into(),
            description: "The player's total net worth, wallet, and skill points aggregated across \
                          all their characters, with a per-character breakdown."
                .into(),
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "portfolio_trend".into(),
            description: "Net-worth change over the last N days from persisted local snapshots. \
                          Use to answer how the player's wealth is trending."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": { "days": { "type": "integer", "description": "Look-back window in days (default 30)" } }
            }),
        },
        ToolSpec {
            name: "rank_income".into(),
            description: "Rank income activities by risk-adjusted ISK/hr over the time available. \
                          Supply the activities you are comparing with their gross ISK/hr."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "hours": { "type": "number", "description": "Hours available" },
                    "activities": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "isk_per_hour": { "type": "number" },
                                "risk": { "type": "number", "description": "0.0-1.0 expected loss fraction" },
                                "setup_cost": { "type": "number" },
                                "eligible": { "type": "boolean" }
                            },
                            "required": ["name", "isk_per_hour"]
                        }
                    }
                },
                "required": ["hours", "activities"]
            }),
        },
        ToolSpec {
            name: "skill_roi".into(),
            description: "Rank candidate skill plans by ISK return on training time. Supply each \
                          plan's training time and the income it unlocks."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "plans": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": { "type": "string" },
                                "train_seconds": { "type": "integer" },
                                "isk_per_hour": { "type": "number" },
                                "hours_per_day": { "type": "number" },
                                "upfront_isk": { "type": "number" }
                            },
                            "required": ["label", "train_seconds", "isk_per_hour", "hours_per_day"]
                        }
                    }
                },
                "required": ["plans"]
            }),
        },
    ]
}

/// Format durable memory notes into a system-message body for recall, or `None`
/// when there's nothing worth injecting. Caps the count so context stays small.
/// Pure.
pub fn memory_context(notes: &[(String, String, String)]) -> Option<String> {
    if notes.is_empty() {
        return None;
    }
    let mut out = String::from(
        "What you remember about this player (use it to personalize advice; do not repeat it back verbatim):\n",
    );
    for (kind, title, body) in notes.iter().take(20) {
        out.push_str(&format!("- [{kind}] {title}: {body}\n"));
    }
    Some(out)
}

/// Shared guardrail clause appended to every agent's prompt.
const GUARDRAILS: &str = " Call the provided tools to fetch real numbers — never invent prices, \
     kills, or risk figures; the tools do the math and you explain the result concisely. You cannot \
     take in-game actions; recommend, and let the player act. Prices are in ISK.";

/// One specialist agent: a focused persona over a subset of the tool registry.
pub struct AgentDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub persona: &'static str,
    /// Tool names this agent may call; empty = all tools (the Commander).
    pub tools: &'static [&'static str],
}

/// The agent roster. The Commander has every tool; specialists are scoped to
/// their domain so they stay focused and cheap.
pub fn agents() -> &'static [AgentDef] {
    &[
        AgentDef {
            id: "commander",
            name: "Commander",
            description: "General orchestrator — routes across every domain.",
            persona: "You are the EVE Commander, the player's all-round advisor.",
            tools: &[],
        },
        AgentDef {
            id: "market",
            name: "Market Analyst",
            description: "Prices, arbitrage, what-to-haul.",
            persona: "You are a sharp EVE market analyst focused on prices, spreads, and hauling.",
            tools: &["search_item", "compare_hubs", "scan_arbitrage"],
        },
        AgentDef {
            id: "intel",
            name: "Threat Analyst",
            description: "Is it safe to undock / fly this route?",
            persona: "You are a cautious EVE intel analyst; you assess danger and advise on safety.",
            tools: &["system_risk"],
        },
        AgentDef {
            id: "fitting",
            name: "Fitting Coach",
            description: "Critique and compare ship fits.",
            persona: "You are an expert EVE fitting coach; you read EHP/DPS/cap and suggest improvements.",
            tools: &["fit_stats", "search_item"],
        },
        AgentDef {
            id: "wealth",
            name: "Accounting Analyst",
            description: "Net worth, wealth trend, income choices.",
            persona: "You are the player's EVE accountant; you explain wealth, trend, and income options.",
            tools: &["account_overview", "portfolio_trend", "rank_income"],
        },
        AgentDef {
            id: "skills",
            name: "Skills Mentor",
            description: "Skill-plan ROI and training priorities.",
            persona: "You are an EVE skills mentor; you rank training by ISK impact and goals.",
            tools: &["skill_roi"],
        },
    ]
}

/// Look up an agent by id, falling back to the Commander.
fn agent(id: &str) -> &'static AgentDef {
    agents().iter().find(|a| a.id == id).unwrap_or(&agents()[0])
}

/// The system prompt for an agent (persona + shared guardrails).
pub fn system_prompt_for(agent_id: &str) -> String {
    format!("{}{GUARDRAILS}", agent(agent_id).persona)
}

/// The tool specs an agent may use (all of them for the Commander).
pub fn tool_specs_for(agent_id: &str) -> Vec<ToolSpec> {
    let allowed = agent(agent_id).tools;
    if allowed.is_empty() {
        return tool_specs();
    }
    tool_specs().into_iter().filter(|t| allowed.contains(&t.name.as_str())).collect()
}


/// Run a tool by name with JSON-string arguments. Returns a JSON string result
/// (or a JSON `{"error": ...}` object) for feeding back to the model.
pub async fn execute_tool(state: &AppState, name: &str, arguments: &str) -> String {
    let args: Value = serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
    match name {
        "search_item" => {
            let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            match state.names.search_types(q, 8).await {
                Ok(hits) => json!(hits
                    .into_iter()
                    .map(|t| json!({ "type_id": t.type_id, "name": t.name }))
                    .collect::<Vec<_>>())
                .to_string(),
                Err(e) => err(&e.to_string()),
            }
        }
        "compare_hubs" => {
            let Some(type_id) = args.get("type_id").and_then(|v| v.as_i64()) else {
                return err("missing type_id");
            };
            match state.marketdata.compare(type_id).await {
                Ok(hubs) => json!(hubs).to_string(),
                Err(e) => err(&e.to_string()),
            }
        }
        "scan_arbitrage" => {
            let fees = eve_core::marketdata::TradeFees::default();
            match state.marketdata.arbitrage(&arbitrage_default_types(), fees).await {
                Ok(opps) => json!(opps).to_string(),
                Err(e) => err(&e.to_string()),
            }
        }
        "system_risk" => system_risk_json(state).await,
        "fit_stats" => {
            let eft = args.get("eft").and_then(|v| v.as_str()).unwrap_or("");
            match crate::commands::compute_fit_stats(state, eft, None).await {
                Ok(s) => serde_json::to_string(&s).unwrap_or_else(|e| err(&e.to_string())),
                Err(e) => err(&e),
            }
        }
        "account_overview" => account_overview_json(state).await,
        "portfolio_trend" => {
            let days = args.get("days").and_then(|v| v.as_i64()).unwrap_or(30).max(1);
            portfolio_trend_json(state, days).await
        }
        "rank_income" => {
            let hours = args.get("hours").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let activities: Vec<eve_core::income::IncomeActivity> = args
                .get("activities")
                .and_then(|a| serde_json::from_value(a.clone()).ok())
                .unwrap_or_default();
            json!(eve_core::income::rank_income(&activities, hours)).to_string()
        }
        "skill_roi" => {
            let plans: Vec<eve_core::skillplan::RoiPlan> = args
                .get("plans")
                .and_then(|p| serde_json::from_value(p.clone()).ok())
                .unwrap_or_default();
            json!(eve_core::skillplan::rank_roi(&plans)).to_string()
        }
        other => err(&format!("unknown tool '{other}'")),
    }
}

fn err(msg: &str) -> String {
    json!({ "error": msg }).to_string()
}

/// A small set of liquid items for the arbitrage tool (minerals + common hulls).
fn arbitrage_default_types() -> Vec<i64> {
    vec![34, 35, 36, 37, 38, 39, 40, 11399, 16240, 587, 597, 603, 593]
}

/// Gather the unified system-risk signals for the active character and return a
/// JSON summary, mirroring the `get_system_risk` command.
async fn system_risk_json(state: &AppState) -> String {
    let Ok(characters) = state.db.list_characters().await else {
        return err("no character data");
    };
    let Some(active) = characters.iter().find(|c| c.active) else {
        return err("no active character");
    };
    let Ok(loc) = state.character.location(active.id).await else {
        return err("active character location unavailable");
    };
    let current_id = loc.solar_system_id;
    let Ok(info) = state.universe.system_info(current_id).await else {
        return err("system info unavailable");
    };
    let neighbor_ids: Vec<i64> = state
        .universe
        .neighbors(current_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .take(12)
        .collect();
    let system_kills = state.zkill.system_kill_count(current_id, 3600).await.unwrap_or(0);
    let mut neighbour_kills = 0;
    for nid in neighbor_ids {
        neighbour_kills += state.zkill.system_kill_count(nid, 3600).await.unwrap_or(0);
    }
    let (danger_pilots, caution_pilots) = crate::commands::local_hostile_counts(state).await;
    let inputs = eve_core::intel::RiskInputs {
        system_kills,
        neighbour_kills,
        danger_pilots,
        caution_pilots,
        security: info.security_status,
        gate_camp: system_kills > 3,
    };
    let risk = eve_core::intel::score_system_risk(&inputs);
    json!({
        "system": info.name,
        "score": risk.score,
        "level": risk.level.as_str(),
        "reasons": risk.reasons,
    })
    .to_string()
}

/// Aggregate net worth / wallet / SP across all characters (mirrors the
/// `get_account_overview` command).
async fn account_overview_json(state: &AppState) -> String {
    let Ok(characters) = state.db.list_characters().await else {
        return err("no character data");
    };
    let prices = state.prices.price_map().await.unwrap_or_default();
    let mut worths = Vec::with_capacity(characters.len());
    for c in characters {
        let (wallet, skills, holdings) = tokio::join!(
            state.character.wallet_balance(c.id),
            state.character.skills(c.id),
            state.assets.all_holdings(c.id),
        );
        let wallet_balance = wallet.unwrap_or(0.0);
        let total_sp = skills.map(|s| s.total_sp).unwrap_or(0);
        let asset_value = holdings
            .map(|groups| eve_core::assets::value_holdings(&groups, &prices, 0).total_value)
            .unwrap_or(0.0);
        worths.push(eve_core::account::CharacterWorth::new(
            c.id,
            c.name,
            wallet_balance,
            asset_value,
            total_sp,
        ));
    }
    json!(eve_core::account::aggregate(worths)).to_string()
}

/// Net-worth trend over `days` from persisted snapshots (account-wide).
async fn portfolio_trend_json(state: &AppState, days: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let since = now - days * 86_400;
    let Ok(points) = state.db.snapshots_total("networth", since).await else {
        return err("no snapshot data yet");
    };
    let (change, pct) = match (points.first(), points.last()) {
        (Some(f), Some(l)) if points.len() >= 2 => {
            let c = l.value - f.value;
            (c, if f.value > 0.0 { c / f.value * 100.0 } else { 0.0 })
        }
        _ => (0.0, 0.0),
    };
    json!({
        "days": days,
        "samples": points.len(),
        "start_value": points.first().map(|p| p.value).unwrap_or(0.0),
        "end_value": points.last().map(|p| p.value).unwrap_or(0.0),
        "change": change,
        "change_pct": pct,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_context_none_when_empty() {
        assert!(memory_context(&[]).is_none());
    }

    #[test]
    fn agents_reference_only_real_tools() {
        let names: std::collections::HashSet<String> =
            tool_specs().into_iter().map(|t| t.name).collect();
        for a in agents() {
            for t in a.tools {
                assert!(names.contains(*t), "agent '{}' references unknown tool '{}'", a.id, t);
            }
            // Specialists are a strict subset; Commander has all.
            let n = tool_specs_for(a.id).len();
            if a.tools.is_empty() {
                assert_eq!(n, tool_specs().len());
            } else {
                assert_eq!(n, a.tools.len());
            }
        }
    }

    #[test]
    fn memory_context_lists_notes() {
        let notes = vec![
            ("goal".into(), "Carrier".into(), "Train toward a Nyx".into()),
            ("preference".into(), "Lowsec".into(), "Avoids lowsec".into()),
        ];
        let ctx = memory_context(&notes).unwrap();
        assert!(ctx.contains("[goal] Carrier: Train toward a Nyx"));
        assert!(ctx.contains("[preference] Lowsec"));
    }
}
