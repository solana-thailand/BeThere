//! Guard: the claim-lock DO paths must not `?` out before their mandatory KV write.
//!
//! Three separate fixes on this branch closed the same defect shape in
//! `claim/lock.rs`: a durable-store call that returned early on failure and so
//! skipped the KV write two lines below a comment promising it would always
//! happen. The KV record is what the attendee actually reads (`finalize`, their
//! `asset_id`/`signature` proof link) and what actually blocks their retry
//! (`release`, a 300-second TTL), so it must be written even when the durable
//! side fails — including when the RPC fails in transport rather than returning
//! `success: false`.
//!
//! `do_rpc_err` is the shape that enforces this: it flattens both failure modes
//! into an `Option<String>` the caller carries *past* the KV write. A bare
//! `do_rpc(..).await?` inside `finalize_claim_lock` or `release_claim_lock`
//! reintroduces the bug, so this test fails on it.
//!
//! `acquire_claim_lock` is deliberately exempt: there a DO failure *must* abort,
//! because handing out a lock that was never durably recorded is the worse bug.

const SOURCE: &str = include_str!("../src/claim/lock.rs");

/// Body of `fn <name>`, from its signature to the start of the next top-level
/// `async fn` / `fn` / `struct` — good enough for a single flat module.
fn fn_body<'a>(source: &'a str, name: &str) -> &'a str {
    let needle = format!("fn {name}(");
    let start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("`fn {name}` not found in claim/lock.rs — guard is stale"));
    let rest = &source[start + needle.len()..];
    let end = rest
        .match_indices("\n}")
        .next()
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn cleanup_paths_do_not_short_circuit_on_a_do_rpc_failure() {
    for name in ["finalize_claim_lock", "release_claim_lock"] {
        let body = fn_body(SOURCE, name);

        assert!(
            body.contains("do_rpc_err("),
            "{name} no longer routes its DO call through `do_rpc_err`; a transport \
             failure would skip the KV write it is required to perform"
        );

        // `do_rpc_err(` contains `do_rpc(`-like text only as a prefix, so strip
        // the helper calls before looking for a bare one.
        let bare = body.replace("do_rpc_err(", "");
        assert!(
            !bare.contains("do_rpc("),
            "{name} calls `do_rpc` directly; use `do_rpc_err` so the DO error is \
             carried past the mandatory KV write instead of returning early"
        );
    }
}

#[test]
fn acquire_still_aborts_on_a_do_failure() {
    let body = fn_body(SOURCE, "acquire_claim_lock");
    assert!(
        body.contains("do_rpc(") && body.contains(".await?"),
        "acquire_claim_lock must keep failing closed on a DO error — it may not \
         hand out a lock that was never durably recorded"
    );
}

#[test]
fn the_helper_flattens_both_failure_modes() {
    let body = fn_body(SOURCE, "do_rpc_err");
    assert!(
        body.contains("Err(e) => Some(e)"),
        "do_rpc_err must map a transport error into the returned Option, not drop it"
    );
    assert!(
        body.contains("resp.success"),
        "do_rpc_err must also treat a `success: false` response as an error"
    );
}
