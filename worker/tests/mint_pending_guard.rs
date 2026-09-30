//! Issue 180 source guard: both resumable claim mint paths map a pending mint
//! to a 504 through `MintError::into_resumable_app_error`, and the poll timeout
//! is typed as `MintError::Pending`. A return to a flat 502 turns this red.

use std::fs;
use std::path::PathBuf;

fn source(path: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path))
        .expect("read worker source")
}

#[test]
fn claim_mint_paths_map_pending_to_a_retryable_error() {
    for path in ["src/claim/mint/execute.rs", "src/claim/mint/walkin.rs"] {
        let src = source(path);
        assert!(
            src.contains(".into_resumable_app_error(\"crossmint\")"),
            "{path} no longer maps a pending mint to UpstreamPending"
        );
        assert!(
            !src.contains("service: \"crossmint\".into(),\n                status: 502"),
            "{path} builds a flat crossmint 502 again"
        );
    }
}

#[test]
fn poll_timeout_is_pending_only_when_the_mint_can_resume() {
    let src = source("src/solana.rs");
    assert!(src.contains("Err(MintError::Pending(detail))"));
    assert!(src.contains("idempotent_id.is_some() || pending_kv.is_some()"));
}
