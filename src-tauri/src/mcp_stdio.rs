//! MCP stdio transport bridge.
//!
//! Exposes EVE Commander's read-only tool registry over a newline-delimited
//! JSON-RPC stdio stream so the player's *own* agents (Claude Desktop, a local
//! agent runtime) can drive the app's tools — the "MCP server" half of the
//! Jarvis design. Launch the binary with `--mcp-stdio`; the agent host wires it
//! as a stdio MCP server (one JSON-RPC message per line in, one per line out).
//!
//! The JSON-RPC semantics live in the pure, tested `eve_core::mcp` layer; this
//! is just the I/O loop + tool execution against a headless [`AppState`].

use std::io::{BufRead, Write};

use eve_core::mcp::{handle, parse_request_line, tool_result, McpAction};

use crate::AppState;

/// The server name reported in `initialize`.
const SERVER_NAME: &str = "eve-commander";

/// Run the stdio MCP server against `state` until stdin closes. Reads one
/// JSON-RPC request per line from stdin and writes one response per line to
/// stdout; notifications (no id) produce no output. Blocking — the caller runs
/// this instead of launching the GUI.
pub fn serve(state: AppState) {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let tools = crate::ai_tools::tool_specs();
    let version = env!("CARGO_PKG_VERSION");

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match parse_request_line(&line) {
            Err(err) => Some(err),
            Ok(request) => match handle(&request, &tools, SERVER_NAME, version) {
                McpAction::Reply(v) => Some(v),
                McpAction::None => None,
                McpAction::CallTool { id, name, arguments } => {
                    // Tool execution is async + needs app state; drive it here.
                    let result = tauri::async_runtime::block_on(crate::ai_tools::execute_tool(
                        &state, &name, &arguments,
                    ));
                    let is_error = result.contains("\"error\"");
                    Some(tool_result(id, &result, is_error))
                }
            },
        };
        if let Some(v) = response {
            if writeln!(stdout, "{v}").is_err() {
                break;
            }
            let _ = stdout.flush();
        }
    }
}
