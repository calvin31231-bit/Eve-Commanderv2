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
    ]
}

/// The system prompt framing the assistant's role and guardrails.
pub fn system_prompt() -> String {
    "You are the EVE Commander assistant, an advisor inside an EVE Online companion app. \
     You help the player understand their data and make decisions. Call the provided tools to \
     fetch real numbers — never invent prices, kills, or risk figures. The tools do the math; you \
     explain the result concisely. You cannot take in-game actions; recommend, and let the player \
     act. Prices are in ISK."
        .to_string()
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
    let inputs = eve_core::intel::RiskInputs {
        system_kills,
        neighbour_kills,
        danger_pilots: 0,
        caution_pilots: 0,
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
