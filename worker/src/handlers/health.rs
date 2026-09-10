use axum::extract::State;
use axum::response::Json;
use serde_json::{Value, json};

use crate::state::AppState;

/// Health check endpoint.
/// Returns service status, explicit Solana network roles, and D1 connectivity.
#[worker::send]
pub async fn health_check(State(state): State<AppState>) -> Json<Value> {
    let networks = network_readiness(
        &state.config.solana.rpc_url,
        &state.config.solana.crossmint_host,
        crate::solana_escrow::cluster(),
        !state.config.solana.api_key.is_empty(),
        !state.config.solana.crossmint_api_key.is_empty(),
        !state.config.solana.crossmint_collection_id.is_empty(),
    );

    // D1 connectivity check — runs a lightweight COUNT query.
    // Wrapped in `worker::send` compatible future.
    let d1_status = match state.d1 {
        Some(db) => {
            let db = Arc::clone(&db);
            let result = check_d1_health(&db).await;
            json!({
                "connected": result.is_ok(),
                "counts": result.unwrap_or_default(),
            })
        }
        None => json!({
            "connected": false,
            "error": "D1 binding not configured",
        }),
    };

    Json(json!({
        "status": "ok",
        "service": "event-checkin",
        "runtime": "cloudflare-workers",
        "version": env!("CARGO_PKG_VERSION"),
        // Backward compatibility for wallet signing and explorer links. This
        // must describe the escrow network, where user transactions are sent.
        "cluster": networks.escrow_cluster.clone(),
        "solana": networks,
        "dev_mode": state.config.dev_mode,
        "d1": d1_status,
    }))
}

#[derive(Debug, serde::Serialize)]
struct NetworkReadiness {
    rpc_cluster: &'static str,
    nft_cluster: &'static str,
    escrow_cluster: String,
    rpc_configured: bool,
    nft_configured: bool,
    consistent: bool,
    warnings: Vec<&'static str>,
}

fn rpc_cluster(url: &str) -> &'static str {
    if url.contains("mainnet") {
        "mainnet-beta"
    } else if url.contains("testnet") {
        "testnet"
    } else if url.contains("devnet") {
        "devnet"
    } else {
        "unknown"
    }
}

fn nft_cluster(host: &str) -> &'static str {
    match host.trim_end_matches('/') {
        "www.crossmint.com" => "mainnet-beta",
        "staging.crossmint.com" => "devnet",
        _ => "unknown",
    }
}

fn network_readiness(
    rpc_url: &str,
    crossmint_host: &str,
    escrow_cluster: &str,
    rpc_key_present: bool,
    crossmint_key_present: bool,
    collection_present: bool,
) -> NetworkReadiness {
    let rpc_cluster = rpc_cluster(rpc_url);
    let nft_cluster = nft_cluster(crossmint_host);
    let rpc_configured = rpc_cluster != "unknown" && rpc_key_present;
    let nft_configured = nft_cluster != "unknown" && crossmint_key_present && collection_present;
    let mut warnings = Vec::new();
    if rpc_cluster != escrow_cluster {
        warnings.push("rpc_cluster_mismatch");
    }
    if nft_cluster != escrow_cluster {
        warnings.push("nft_cluster_mismatch");
    }
    if !rpc_configured {
        warnings.push("rpc_not_configured");
    }
    if !nft_configured {
        warnings.push("nft_not_configured");
    }
    NetworkReadiness {
        rpc_cluster,
        nft_cluster,
        escrow_cluster: escrow_cluster.to_string(),
        rpc_configured,
        nft_configured,
        consistent: warnings.is_empty(),
        warnings,
    }
}

use std::sync::Arc;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct D1Counts {
    attendees: i64,
    contacts: i64,
    events: i64,
    staff: i64,
    claim_locks: i64,
    audit_log: i64,
}

async fn check_d1_health(db: &worker::D1Database) -> Result<D1Counts, String> {
    let stmt = db.prepare(
        "SELECT \
         (SELECT COUNT(*) FROM attendees) as attendees, \
         (SELECT COUNT(*) FROM contacts) as contacts, \
         (SELECT COUNT(*) FROM events) as events, \
         (SELECT COUNT(*) FROM staff) as staff, \
         (SELECT COUNT(*) FROM claim_locks) as claim_locks, \
         (SELECT COUNT(*) FROM audit_log) as audit_log",
    );
    let row = stmt
        .first::<D1Counts>(None)
        .await
        .map_err(|e| format!("D1 health query: {e:?}"))?;

    Ok(row.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::network_readiness;

    #[test]
    fn reports_each_network_role_without_secrets() {
        let readiness = network_readiness(
            "https://mainnet.helius-rpc.com",
            "staging.crossmint.com",
            "devnet",
            true,
            true,
            true,
        );
        assert_eq!(readiness.rpc_cluster, "mainnet-beta");
        assert_eq!(readiness.nft_cluster, "devnet");
        assert_eq!(readiness.escrow_cluster, "devnet");
        assert!(!readiness.consistent);
        assert_eq!(readiness.warnings, ["rpc_cluster_mismatch"]);
    }

    #[test]
    fn flags_unknown_or_incomplete_nft_configuration() {
        let readiness = network_readiness(
            "https://devnet.helius-rpc.com",
            "crossmint.invalid",
            "devnet",
            true,
            false,
            false,
        );
        assert!(!readiness.nft_configured);
        assert!(readiness.warnings.contains(&"nft_cluster_mismatch"));
        assert!(readiness.warnings.contains(&"nft_not_configured"));
    }

    #[test]
    fn matching_complete_configuration_is_ready() {
        let readiness = network_readiness(
            "https://devnet.helius-rpc.com",
            "staging.crossmint.com/",
            "devnet",
            true,
            true,
            true,
        );
        assert!(readiness.consistent);
        assert!(readiness.warnings.is_empty());
    }
}
