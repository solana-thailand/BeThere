//! `/events` (.plans/045 R4.3): the open events live in its head and
//! nowhere else, so a visitor never sees the same list twice.

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn events_page_runs_head_mine_learn_deposit() {
    let page = read("src/pages/site/events.rs");
    let order = ["<OpenEvents", "<DiscoverList", "<Learn", "<DepositWalk"];
    let at: Vec<usize> = order
        .iter()
        .map(|m| page.find(m).unwrap_or_else(|| panic!("{m}")))
        .collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "order {order:?}");
}

#[test]
fn the_open_list_is_fetched_once() {
    // Discover keeps only the reader's own events.
    assert!(!read("src/pages/public/discover.rs").contains("/public/events"));
    // The home links to /events instead of listing them.
    let home = read("src/pages/landing/page.rs");
    assert!(!home.contains("OpenEvents") && !home.contains("Upcoming"));
    assert!(read("src/pages/landing/hero.rs").contains(r#"<A href="/events""#));
}

/// R4.13: the empty state tells a reader with credit that it can pay their
/// next deposit, through the read that never sends a signed-out visitor to
/// /login.
#[test]
fn empty_state_shows_held_credit_without_a_login_redirect() {
    let src = read("src/pages/site/open_events.rs");
    let empty = &src[src.find("fn NothingOpen").unwrap()..];
    assert!(empty.contains("<CreditHeld />"));
    assert!(src.contains("crate::api::get_credit_balance()"));
    assert!(src.contains("(amount > 0).then("));
}
