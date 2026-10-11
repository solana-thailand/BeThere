//! One RPC per purpose (2026-10-11): `HELIUS_RPC_URL` serves badge reads and
//! may name another cluster than the escrow (prod: mainnet badges, devnet
//! escrow), so no worker code may call the ambiguous `full_rpc_url()`. The
//! escrow and USDC paths use `solana_escrow::rpc_url` (by `SOLANA_CLUSTER`),
//! the sandbox `devnet_rpc_url()`.

use std::path::Path;

fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        match path.is_dir() {
            true => sources(&path, out),
            false if path.extension().is_some_and(|e| e == "rs") => {
                out.push((
                    path.display().to_string(),
                    std::fs::read_to_string(&path).unwrap(),
                ));
            }
            false => {}
        }
    }
}

#[test]
fn no_worker_code_uses_the_ambiguous_rpc() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let offenders: Vec<&str> = files
        .iter()
        .filter(|(_, src)| src.contains("full_rpc_url("))
        .map(|(path, _)| path.as_str())
        .collect();
    assert!(
        offenders.is_empty(),
        "use solana_escrow::rpc_url: {offenders:?}"
    );
}

#[test]
fn escrow_and_usdc_paths_use_the_cluster_rpc() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers");
    for rel in [
        "deposit/escrow/handlers.rs",
        "deposit/escrow/status.rs",
        "deposit/usdc/handlers/tx.rs",
        "deposit/usdc/recover.rs",
        "escrow_index.rs",
        "events/update.rs",
        "health.rs",
    ] {
        let src = std::fs::read_to_string(root.join(rel)).unwrap();
        assert!(
            src.contains("crate::solana_escrow::rpc_url(&state.config.solana)"),
            "{rel} lost the cluster RPC"
        );
    }
}
