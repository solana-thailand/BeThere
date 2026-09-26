//! Offline tests: protocol dispatch, tool catalogue, config guards and the
//! argument checks that stand between an agent and a URL path. No network.

use bethere_mcp::config::{Config, DEFAULT_MAX_DEPOSIT_USDC};
use bethere_mcp::server::{self, Incoming, PROTOCOL_VERSION};
use bethere_mcp::tools::{path_segment, ToolName, Tools};
use serde_json::{json, Value};

fn offline_tools() -> Tools {
    let config = Config::from_values(None, None, None, None).expect("defaults are valid");
    Tools::new(config, None).expect("client builds")
}

#[test]
fn notification_gets_no_reply() {
    let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    assert_eq!(server::classify(line), Incoming::Notification);
}

#[test]
fn garbage_is_a_parse_error() {
    match server::classify("{not json") {
        Incoming::Error { code, .. } => assert_eq!(code, -32700),
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn unknown_method_and_unknown_tool_are_errors() {
    match server::classify(r#"{"jsonrpc":"2.0","id":1,"method":"resources/list"}"#) {
        Incoming::Error { code, .. } => assert_eq!(code, -32601),
        other => panic!("expected method-not-found, got {other:?}"),
    }
    match server::classify(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"drain_wallet"}}"#,
    ) {
        Incoming::Error { code, .. } => assert_eq!(code, -32602),
        other => panic!("expected invalid-params, got {other:?}"),
    }
}

#[test]
fn tool_call_is_classified_with_arguments() {
    let line = r#"{"jsonrpc":"2.0","id":"a","method":"tools/call","params":{"name":"event_details","arguments":{"slug":"rtm-6"}}}"#;
    assert_eq!(
        server::classify(line),
        Incoming::CallTool {
            id: json!("a"),
            tool: ToolName::EventDetails,
            args: json!({ "slug": "rtm-6" }),
        }
    );
}

#[test]
fn initialize_advertises_tools_only() {
    let result = server::initialize_result(Some("1999-01-01"));
    assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["capabilities"].get("resources").is_none());
    assert_eq!(result["serverInfo"]["name"], "bethere-mcp");
}

#[test]
fn tool_catalogue_is_complete_and_well_formed() {
    let list = server::list_tools_result();
    let tools = list["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), ToolName::ALL.len());
    let mut names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), tools.len(), "tool names must be unique");
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert!(!tool["description"].as_str().unwrap_or("").is_empty());
        let name = tool["name"].as_str().unwrap();
        assert_eq!(ToolName::parse(name).map(ToolName::as_str), Some(name));
    }
    let register = tools.iter().find(|t| t["name"] == "register").unwrap();
    let required: Vec<&str> = register["inputSchema"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(required.contains(&"consent_given"));
}

#[test]
fn config_refuses_prod_worker_and_mainnet_rpc() {
    assert!(Config::from_values(
        Some("https://bethere.solana-thailand.workers.dev"),
        None,
        None,
        None
    )
    .is_err());
    assert!(Config::from_values(
        None,
        Some("https://api.mainnet-beta.solana.com"),
        None,
        None
    )
    .is_err());
    assert!(Config::from_values(Some("http://localhost:8787"), None, None, None).is_ok());
}

#[test]
fn config_spend_cap_defaults_and_parses() {
    let cfg = Config::from_values(None, None, None, None).unwrap();
    assert_eq!(cfg.max_deposit_usdc, DEFAULT_MAX_DEPOSIT_USDC);
    let cfg = Config::from_values(None, None, None, Some("2500000")).unwrap();
    assert_eq!(cfg.max_deposit_usdc, 2_500_000);
    assert!(Config::from_values(None, None, None, Some("ten")).is_err());
}

#[test]
fn path_segments_cannot_steer_the_route() {
    for bad in [
        "../admin", "a/b", "a?b=c", "a#b", "..", ".hidden", "", "a b",
    ] {
        assert!(
            path_segment(&json!({ "slug": bad }), "slug").is_err(),
            "accepted {bad:?}"
        );
    }
    for good in ["rtm-6", "flow-deposit-20260913", "evt_01.x"] {
        assert_eq!(
            path_segment(&json!({ "slug": good }), "slug").unwrap(),
            good
        );
    }
}

#[tokio::test]
async fn wallet_tools_explain_missing_keypair_without_network() {
    let tools = offline_tools();
    let line = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"agent_wallet","arguments":{}}}"#;
    let reply = server::handle_line(&tools, line).await.expect("a reply");
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["result"]["isError"], true);
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("BETHERE_AGENT_KEYPAIR"), "{text}");
}

#[tokio::test]
async fn register_requires_consent_before_any_request() {
    let tools = offline_tools();
    let args =
        json!({ "slug": "x", "name": "A", "email": "a@example.com", "consent_given": false });
    let err = tools
        .call(ToolName::Register, &args)
        .await
        .expect_err("must refuse");
    assert!(err.to_string().contains("consent_given"), "{err}");
}
