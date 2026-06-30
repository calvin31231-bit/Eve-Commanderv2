//! Minimal Model Context Protocol (MCP) server handler.
//!
//! Exposes the AI tool registry to the player's *own* agents (Claude Desktop, a
//! local runtime, …) so EVE Commander becomes a tool surface they can drive —
//! the two-way half of the Jarvis design. This is the pure JSON-RPC 2.0 layer:
//! it parses a request and either returns a ready response (initialize /
//! tools/list / ping) or asks the caller to run a tool (tools/call), which is
//! async + needs app state. Transport (stdio/HTTP) wraps this.

use serde_json::{json, Value};

use crate::ai::ToolSpec;

/// MCP protocol revision this server speaks.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// What the transport should do after [`handle`] inspects a request.
pub enum McpAction {
    /// A finished JSON-RPC response to send back.
    Reply(Value),
    /// A `tools/call`: run `name` with `arguments` (a JSON string), then wrap the
    /// result with [`tool_result`] using `id`.
    CallTool { id: Value, name: String, arguments: String },
    /// A notification (no response expected).
    None,
}

/// JSON-RPC error response.
pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Wrap a tool's string result into a `tools/call` response. `is_error` flags a
/// tool-level failure (still a valid JSON-RPC result, per MCP).
pub fn tool_result(id: Value, result_text: &str, is_error: bool) -> Value {
    ok(
        id,
        json!({
            "content": [{ "type": "text", "text": result_text }],
            "isError": is_error,
        }),
    )
}

/// Parse one line of newline-delimited JSON-RPC from a transport (stdio). On
/// success returns the request value; on malformed JSON returns a ready
/// JSON-RPC parse-error response (code -32700) for the transport to emit. Pure.
pub fn parse_request_line(line: &str) -> std::result::Result<Value, Value> {
    match serde_json::from_str::<Value>(line) {
        Ok(v) => Ok(v),
        Err(e) => Err(error(Value::Null, -32700, &format!("parse error: {e}"))),
    }
}

/// Handle one parsed JSON-RPC request. Pure. `tools` is the offered registry;
/// `server_name`/`version` identify this server in `initialize`.
pub fn handle(request: &Value, tools: &[ToolSpec], server_name: &str, version: &str) -> McpAction {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => McpAction::Reply(ok(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": server_name, "version": version },
            }),
        )),
        // Notifications carry no id and expect no reply.
        m if m.starts_with("notifications/") => McpAction::None,
        "ping" => McpAction::Reply(ok(id, json!({}))),
        "tools/list" => {
            let list: Vec<Value> = tools
                .iter()
                .map(|t| json!({ "name": t.name, "description": t.description, "inputSchema": t.parameters }))
                .collect();
            McpAction::Reply(ok(id, json!({ "tools": list })))
        }
        "tools/call" => {
            let params = request.get("params");
            let name = params
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() || !tools.iter().any(|t| t.name == name) {
                return McpAction::Reply(error(id, -32602, "unknown tool"));
            }
            // Arguments come as an object; tools take a JSON string.
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .map(|a| a.to_string())
                .unwrap_or_else(|| "{}".to_string());
            McpAction::CallTool { id, name, arguments }
        }
        "" => McpAction::Reply(error(id, -32600, "invalid request: no method")),
        other => McpAction::Reply(error(id, -32601, &format!("method not found: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<ToolSpec> {
        vec![ToolSpec {
            name: "system_risk".into(),
            description: "risk".into(),
            parameters: json!({ "type": "object", "properties": {} }),
        }]
    }

    #[test]
    fn initialize_reports_server_info() {
        let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" });
        let McpAction::Reply(r) = handle(&req, &tools(), "eve-commander", "0.1.0") else {
            panic!("expected reply");
        };
        assert_eq!(r["result"]["serverInfo"]["name"], "eve-commander");
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn tools_list_returns_registry() {
        let req = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
        let McpAction::Reply(r) = handle(&req, &tools(), "s", "v") else { panic!() };
        assert_eq!(r["result"]["tools"][0]["name"], "system_risk");
        assert!(r["result"]["tools"][0]["inputSchema"].is_object());
    }

    #[test]
    fn tools_call_routes_to_executor() {
        let req = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "system_risk", "arguments": {} } });
        match handle(&req, &tools(), "s", "v") {
            McpAction::CallTool { name, .. } => assert_eq!(name, "system_risk"),
            _ => panic!("expected CallTool"),
        }
        // Wrapping a result.
        let wrapped = tool_result(json!(3), "{\"score\":42}", false);
        assert_eq!(wrapped["result"]["content"][0]["text"], "{\"score\":42}");
        assert_eq!(wrapped["result"]["isError"], false);
    }

    #[test]
    fn unknown_tool_and_method_error() {
        let bad_tool = json!({ "id": 4, "method": "tools/call", "params": { "name": "nope" } });
        let McpAction::Reply(r) = handle(&bad_tool, &tools(), "s", "v") else { panic!() };
        assert_eq!(r["error"]["code"], -32602);

        let bad_method = json!({ "id": 5, "method": "frobnicate" });
        let McpAction::Reply(r) = handle(&bad_method, &tools(), "s", "v") else { panic!() };
        assert_eq!(r["error"]["code"], -32601);
    }

    #[test]
    fn notification_has_no_reply() {
        let note = json!({ "method": "notifications/initialized" });
        assert!(matches!(handle(&note, &tools(), "s", "v"), McpAction::None));
    }

    #[test]
    fn parse_request_line_ok_and_error() {
        let ok = parse_request_line("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}").unwrap();
        assert_eq!(ok["method"], "ping");
        // Malformed JSON yields a -32700 parse-error reply, not a panic.
        let err = parse_request_line("{not json").unwrap_err();
        assert_eq!(err["error"]["code"], -32700);
        assert_eq!(err["id"], Value::Null);
    }
}
