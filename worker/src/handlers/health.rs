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

    // D1 connectivity only. Row counts were public and cost a full scan of six
    // tables per anonymous hit (issue 147); `SELECT 1` reads no rows.
    let d1_status = match state.d1 {
        Some(db) => json!({ "connected": d1_reachable(&db).await }),
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

async fn d1_reachable(db: &worker::D1Database) -> bool {
    db.prepare("SELECT 1 AS ok")
        .first::<serde_json::Value>(None)
        .await
        .is_ok_and(|row| row.is_some())
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
