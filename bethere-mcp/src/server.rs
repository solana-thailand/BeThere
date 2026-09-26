//! MCP over stdio: newline-delimited JSON-RPC 2.0. Only the tools capability
//! is served, so the protocol surface is `initialize`, `ping`, `tools/list`
//! and `tools/call`. Stdout carries protocol messages only; logs go to stderr.

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::tools::{ToolName, Tools};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// What one incoming line asks for, decided before any tool runs.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    /// A notification (no `id`): never answered.
    Notification,
    Initialize {
        id: Value,
        client_version: Option<String>,
    },
    Ping {
        id: Value,
    },
    ListTools {
        id: Value,
    },
    CallTool {
        id: Value,
        tool: ToolName,
        args: Value,
    },
    /// Answer immediately with a JSON-RPC error.
    Error {
        id: Value,
        code: i64,
        message: String,
    },
}

pub fn classify(line: &str) -> Incoming {
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return Incoming::Error {
                id: Value::Null,
                code: PARSE_ERROR,
                message: format!("parse error: {e}"),
            }
        }
    };
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        return Incoming::Error {
            id: msg.get("id").cloned().unwrap_or(Value::Null),
            code: INVALID_REQUEST,
            message: "missing method".to_string(),
        };
    };
    let Some(id) = msg.get("id").cloned() else {
        return Incoming::Notification;
    };
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "initialize" => Incoming::Initialize {
            id,
            client_version: params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .map(str::to_string),
        },
        "ping" => Incoming::Ping { id },
        "tools/list" => Incoming::ListTools { id },
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            match ToolName::parse(name) {
                Some(tool) => Incoming::CallTool {
                    id,
                    tool,
                    args: params
                        .get("arguments")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                },
                None => Incoming::Error {
                    id,
                    code: INVALID_PARAMS,
                    message: format!("unknown tool '{name}'"),
                },
            }
        }
        other => Incoming::Error {
            id,
            code: METHOD_NOT_FOUND,
            message: format!("method not found: {other}"),
        },
    }
}

pub fn initialize_result(client_version: Option<&str>) -> Value {
    // Echo a version the client asked for only if it is ours; otherwise offer
    // ours and let the client decide (MCP version negotiation).
    let version = match client_version {
        Some(v) if v == PROTOCOL_VERSION => v,
        _ => PROTOCOL_VERSION,
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "bethere-mcp", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "BeThere: find an event, register a person, and commit to showing up by paying the USDC deposit into the Solana devnet escrow from the agent's own wallet. Typical order: find_events → event_details → register → pay_deposit → ticket_status.",
    })
}

pub fn list_tools_result() -> Value {
    json!({ "tools": ToolName::ALL.map(ToolName::definition) })
}

fn tool_result(outcome: Result<Value, crate::error::ToolError>) -> Value {
    match outcome {
        Ok(value) => json!({
            "content": [{ "type": "text", "text": value.to_string() }],
            "structuredContent": wrap_structured(value),
            "isError": false,
        }),
        Err(e) => json!({
            "content": [{ "type": "text", "text": e.to_string() }],
            "isError": true,
        }),
    }
}

/// `structuredContent` must be an object; wrap arrays and scalars.
fn wrap_structured(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        other => json!({ "result": other }),
    }
}

fn response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Handle one line; `None` for notifications.
pub async fn handle_line(tools: &Tools, line: &str) -> Option<Value> {
    match classify(line) {
        Incoming::Notification => None,
        Incoming::Initialize { id, client_version } => {
            Some(response(id, initialize_result(client_version.as_deref())))
        }
        Incoming::Ping { id } => Some(response(id, json!({}))),
        Incoming::ListTools { id } => Some(response(id, list_tools_result())),
        Incoming::CallTool { id, tool, args } => {
            eprintln!("bethere-mcp: tools/call {}", tool.as_str());
            Some(response(id, tool_result(tools.call(tool, &args).await)))
        }
        Incoming::Error { id, code, message } => Some(error_response(id, code, message)),
    }
}

/// Serve requests from stdin until EOF. Requests are handled one at a time;
/// a deposit must not race a second deposit for the same attendee.
pub async fn serve<W: AsyncWrite + Unpin>(tools: &Tools, mut out: W) -> std::io::Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(reply) = handle_line(tools, line).await {
            out.write_all(reply.to_string().as_bytes()).await?;
            out.write_all(b"\n").await?;
            out.flush().await?;
        }
    }
    Ok(())
}
