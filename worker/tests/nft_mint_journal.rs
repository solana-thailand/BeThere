use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn repo_file(path: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path))
        .expect("read repository file")
}

#[test]
fn migration_enforces_one_stateful_job_per_event_claim() {
    let migration = repo_file("migrations/0033_nft_mint_jobs.sql");
    for guard in [
        "PRIMARY KEY (event_id, claim_token)",
        "CHECK (status IN ('pending','confirmed','persisted'))",
        "wallet          TEXT NOT NULL",
        "provider_mint_id TEXT NOT NULL",
    ] {
        assert!(migration.contains(guard), "migration lost `{guard}`");
    }

    if !Command::new("sqlite3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        eprintln!("sqlite3 not on PATH; static migration guards still passed");
        return;
    }
    let db = std::env::temp_dir().join(format!("bethere_mint_journal_{}.db", std::process::id()));
    let _ = fs::remove_file(&db);
    let mut child = Command::new("sqlite3")
        .arg(&db)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sqlite3");
    let sql = format!(
        "{migration}\n\
         INSERT INTO nft_mint_jobs(event_id,claim_token,wallet,provider_mint_id,status) VALUES('e','t','w','provider-id','pending');\n\
         UPDATE nft_mint_jobs SET status='confirmed',asset_id='asset',signature='sig' WHERE event_id='e' AND claim_token='t';\n\
         UPDATE nft_mint_jobs SET status='persisted' WHERE event_id='e' AND claim_token='t';\n\
         SELECT status||'|'||asset_id FROM nft_mint_jobs WHERE event_id='e' AND claim_token='t';"
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(sql.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "persisted|asset"
    );
    let _ = fs::remove_file(db);
}

#[test]
fn every_attendee_claim_journals_before_projection() {
    for path in ["src/claim/mint/execute.rs", "src/claim/mint/walkin.rs"] {
        let source = repo_file(path);
        let mint = source.find("mint_with_journal(").expect("journaled mint");
        let projection = source
            .find("db::attendees::claim_attendee(")
            .expect("attendee projection");
        let persisted = source
            .find("mark_projection_persisted(")
            .expect("journal finalization");
        assert!(mint < projection && projection < persisted, "{path}");
        assert!(!source.contains("D1 claim write failed (non-fatal)"));
        assert!(!source.contains("mint succeeded, data may be inconsistent"));
    }
}

#[test]
fn attendee_projection_fails_when_no_row_matches() {
    let source = repo_file("src/db/attendees/writes.rs");
    let function = source
        .split("pub(crate) async fn claim_attendee")
        .nth(1)
        .expect("claim attendee function")
        .split("/// Write deposit verification")
        .next()
        .unwrap();
    assert!(function.contains("if changes == 0"));
    assert!(function.contains("updated no attendee row"));
}

#[test]
fn attendee_mints_use_crossmint_idempotent_put() {
    let source = repo_file("src/solana.rs");
    assert!(source.contains("Method::Put"));
    assert!(source.contains("format!(\"{collection_url}/{id}\")"));
}
