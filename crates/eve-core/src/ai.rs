//! The AI "Jarvis" layer: an OpenAI-compatible chat client plus the tool-call
//! plumbing the orchestrator uses to drive EVE Commander's own commands.
//!
//! Design constraints (see `docs/ROADMAP.md`, Phase 6.5):
//! - **Local-first and off by default.** Works against any OpenAI-compatible
//!   endpoint — Ollama, LM Studio, llama.cpp, vLLM, or a cloud key. With a local
//!   model, the player's data never leaves the machine.
//! - **The model orchestrates; deterministic Rust does the math.** The LLM is
//!   never asked to compute ISK — it calls our typed commands (exposed here as
//!   [`ToolSpec`]s) and narrates structured results.
//!
//! The wire-format (de)serialization is pure and unit-tested; only the actual
//! HTTP round-trip needs a running model, which is exercised on the user's
//! machine.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{Error, Result};

/// A chat role in the OpenAI message schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

/// One message in a conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    /// Set on a `tool` message: which tool call this is the result of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Set on an `assistant` message that requested tool calls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into(), tool_call_id: None, tool_calls: Vec::new() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into(), tool_call_id: None, tool_calls: Vec::new() }
    }
    /// A tool-result message feeding a prior tool call's output back to the model.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: Vec::new(),
        }
    }
}

/// A function the model may call. `parameters` is a JSON-Schema object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// A tool call the model asked for. `arguments` is a JSON string (OpenAI ships
/// the function arguments as a string, not an object).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// The assistant's reply: free text plus any tool calls it requested.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

/// Render one message into the OpenAI wire shape. Pure.
fn message_to_wire(m: &ChatMessage) -> Value {
    let mut obj = json!({ "role": m.role.as_str(), "content": m.content });
    if let Some(id) = &m.tool_call_id {
        obj["tool_call_id"] = json!(id);
    }
    if !m.tool_calls.is_empty() {
        obj["tool_calls"] = Value::Array(
            m.tool_calls
                .iter()
                .map(|tc| {
                    json!({
                        "id": tc.id,
                        "type": "function",
                        "function": { "name": tc.name, "arguments": tc.arguments },
                    })
                })
                .collect(),
        );
    }
    obj
}

/// Build the `/v1/chat/completions` request body. Pure.
pub fn build_chat_body(model: &str, messages: &[ChatMessage], tools: &[ToolSpec]) -> Value {
    let mut body = json!({
        "model": model,
        "messages": messages.iter().map(message_to_wire).collect::<Vec<_>>(),
        "stream": false,
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        },
                    })
                })
                .collect(),
        );
        body["tool_choice"] = json!("auto");
    }
    body
}

/// Parse a `/v1/chat/completions` response into [`ChatResponse`]. Tolerant of a
/// missing `content` (tool-only turns) and of the two ways endpoints encode a
/// tool call's arguments (string or inline object). Pure.
pub fn parse_chat_response(v: &Value) -> Result<ChatResponse> {
    let msg = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .ok_or_else(|| Error::other("AI response missing choices[0].message"))?;

    let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();

    let mut tool_calls = Vec::new();
    if let Some(arr) = msg.get("tool_calls").and_then(|t| t.as_array()) {
        for tc in arr {
            let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
            let func = tc.get("function");
            let name = func
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let arguments = match func.and_then(|f| f.get("arguments")) {
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
                None => "{}".to_string(),
            };
            if !name.is_empty() {
                tool_calls.push(ToolCall { id, name, arguments });
            }
        }
    }

    Ok(ChatResponse { content, tool_calls })
}

/// Common local OpenAI-compatible endpoints, for auto-detect: (label, base_url).
pub const LOCAL_ENDPOINTS: &[(&str, &str)] = &[
    ("Ollama", "http://127.0.0.1:11434/v1"),
    ("LM Studio", "http://127.0.0.1:1234/v1"),
    ("llama.cpp", "http://127.0.0.1:8080/v1"),
    ("Jan", "http://127.0.0.1:1337/v1"),
];

/// An OpenAI-compatible chat client. `base_url` is the API root (e.g.
/// `http://127.0.0.1:11434/v1`); `api_key` is optional for local servers.
#[derive(Clone)]
pub struct AiClient {
    http: reqwest::Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl AiClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, api_key: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_default();
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            api_key,
        }
    }

    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(k) if !k.is_empty() => rb.bearer_auth(k),
            _ => rb,
        }
    }

    /// Send a chat completion (optionally offering tools) and parse the reply.
    pub async fn chat(&self, messages: &[ChatMessage], tools: &[ToolSpec]) -> Result<ChatResponse> {
        let body = build_chat_body(&self.model, messages, tools);
        let url = format!("{}/chat/completions", self.base_url);
        let resp = self.auth(self.http.post(&url).json(&body)).send().await?;
        if !resp.status().is_success() {
            let code = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::other(format!("AI endpoint {code}: {text}")));
        }
        let v: Value = resp.json().await?;
        parse_chat_response(&v)
    }

    /// List the model ids the endpoint advertises (`/v1/models`).
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/models", self.base_url);
        let resp = self.auth(self.http.get(&url)).send().await?;
        if !resp.status().is_success() {
            return Err(Error::other(format!("AI endpoint {} on /models", resp.status())));
        }
        let v: Value = resp.json().await?;
        let ids = v
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_body_with_tools_and_messages() {
        let msgs = vec![ChatMessage::system("You are a helper."), ChatMessage::user("hi")];
        let tools = vec![ToolSpec {
            name: "price_check".into(),
            description: "Look up an item price".into(),
            parameters: json!({ "type": "object", "properties": { "type_id": { "type": "integer" } } }),
        }];
        let body = build_chat_body("local-model", &msgs, &tools);
        assert_eq!(body["model"], "local-model");
        assert_eq!(body["messages"].as_array().unwrap().len(), 2);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["tools"][0]["function"]["name"], "price_check");
        assert_eq!(body["tool_choice"], "auto");
    }

    #[test]
    fn omits_tools_when_none() {
        let body = build_chat_body("m", &[ChatMessage::user("yo")], &[]);
        assert!(body.get("tools").is_none());
        assert!(body.get("tool_choice").is_none());
    }

    #[test]
    fn parses_plain_text_reply() {
        let v = json!({ "choices": [ { "message": { "role": "assistant", "content": "Jita is busy." } } ] });
        let r = parse_chat_response(&v).unwrap();
        assert_eq!(r.content, "Jita is busy.");
        assert!(r.tool_calls.is_empty());
    }

    #[test]
    fn parses_tool_calls_string_and_object_args() {
        let v = json!({
            "choices": [ { "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    { "id": "a", "type": "function", "function": { "name": "price_check", "arguments": "{\"type_id\":34}" } },
                    { "id": "b", "type": "function", "function": { "name": "route", "arguments": { "to": "Jita" } } }
                ]
            } } ]
        });
        let r = parse_chat_response(&v).unwrap();
        assert_eq!(r.content, "");
        assert_eq!(r.tool_calls.len(), 2);
        assert_eq!(r.tool_calls[0].name, "price_check");
        assert_eq!(r.tool_calls[0].arguments, "{\"type_id\":34}");
        // Inline-object arguments are normalized to a JSON string.
        assert_eq!(r.tool_calls[1].name, "route");
        assert!(r.tool_calls[1].arguments.contains("Jita"));
    }

    #[test]
    fn tool_result_message_roundtrips_to_wire() {
        let m = ChatMessage::tool_result("call-1", "247 ISK");
        let wire = message_to_wire(&m);
        assert_eq!(wire["role"], "tool");
        assert_eq!(wire["tool_call_id"], "call-1");
        assert_eq!(wire["content"], "247 ISK");
    }

    #[test]
    fn missing_choices_is_an_error() {
        assert!(parse_chat_response(&json!({})).is_err());
    }
}
