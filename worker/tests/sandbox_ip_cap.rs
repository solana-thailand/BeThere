//! Per-connection sandbox caps (SG3): the faucet and event creation claim a
//! per-IP slot before the day's slot, key it by a keyed fingerprint (never
//! the IP), and give it back when the work does not happen.

const SRC: &str = include_str!("../src/handlers/sandbox.rs");

#[test]
fn per_ip_slot_is_fingerprinted_claimed_first_and_released() {
    assert!(
        SRC.contains("state.log_fingerprint(&ip)"),
        "the key is a fingerprint"
    );
    assert!(
        !SRC.contains("format!(\"sandbox:ip-{kind}:{ip}"),
        "never the raw IP"
    );
    for (kind, global) in [
        ("\"faucet\"", "FAUCET_PREFIX, &item, FAUCET_GRANTS_PER_DAY"),
        ("\"event\"", "EVENT_PREFIX, &item, EVENTS_PER_DAY"),
    ] {
        let ip_at = SRC
            .find(&format!("ip_prefix(&state, &headers, {kind})"))
            .expect(kind);
        let global_at = SRC.find(global).expect(global);
        assert!(
            ip_at < global_at,
            "{kind}: the per-IP slot is claimed before the day's"
        );
    }
    // every refusal or failure after the IP claim gives the slot back
    assert_eq!(
        SRC.matches("release_ip().await").count(),
        3,
        "faucet releases"
    );
    assert_eq!(
        SRC.matches("quota::release(db, &ip, &item).await").count(),
        3,
        "event + helper releases"
    );
}

#[test]
fn caps_are_small_and_below_the_day() {
    use event_checkin_worker::sandbox::config::{
        EVENTS_PER_DAY, FAUCET_GRANTS_PER_DAY, IP_EVENTS_PER_DAY, IP_FAUCET_GRANTS_PER_DAY,
    };
    // checked at compile time: a cap change that breaks this does not build
    const {
        assert!(
            IP_FAUCET_GRANTS_PER_DAY >= 1 && IP_FAUCET_GRANTS_PER_DAY * 10 <= FAUCET_GRANTS_PER_DAY
        );
        assert!(IP_EVENTS_PER_DAY >= 1 && IP_EVENTS_PER_DAY * 10 <= EVENTS_PER_DAY);
    }
}

/// Rent reclaim (SG3): the vault must be empty before anything is signed,
/// a closed event is marked only after the close went through, and the
/// on-demand trigger is super-admin only.
#[test]
fn reclaim_touches_no_deposit_and_is_admin_only() {
    let src = include_str!("../src/sandbox/reclaim.rs");
    let balance = src.find("let held = token_balance(").expect("vault read");
    let empty = src
        .find("if held > 0 {\n        return Ok(false);")
        .expect("holding check");
    let deactivate = src
        .find("build_deactivate_event_transaction(rpc_url")
        .expect("deactivate");
    assert!(
        balance < empty && empty < deactivate,
        "check the vault before signing"
    );
    let closed = src
        .find("Ok(true) => {\n                out.closed += 1;")
        .expect("closed arm");
    assert!(
        src[closed..].find("MARK_SQL").is_some_and(|i| i < 200),
        "mark only after a close"
    );
    let handler = include_str!("../src/handlers/sandbox.rs");
    let at = handler.find("pub async fn reclaim_now(").expect("trigger");
    assert!(handler[at..at + 600].contains("super_admin_emails"));
}
