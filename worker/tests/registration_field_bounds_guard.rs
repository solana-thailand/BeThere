//! Dynamic registration answers must stay bounded and honestly labelled.
//!
//! `profile_fields` is a free-form `HashMap<String, String>` on two **public**
//! endpoints — `POST /api/public/event/{slug}/register` and
//! `.../register-post-event` — so its size and contents are chosen by the
//! caller. Both funnel through `write_developer_data`, which is the only place
//! the bound is applied; a caller that iterates the raw map instead would be
//! unbounded again, and nothing else in the suite would notice.
//!
//! Filed alongside the DevRel phase-2 request to collect post-event
//! satisfaction through this path (`post.` keys).

use std::{fs, path::Path};

const CONTACT: &str = "src/handlers/register/contact.rs";

fn worker_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    let path = worker_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
}

/// Every consumer of the answer map must go through the bounded accessor.
#[test]
fn the_raw_field_map_is_never_iterated_directly() {
    let source = read(CONTACT);

    let raw_loops: Vec<&str> = source
        .lines()
        .filter(|line| {
            line.contains("in &data.profile_fields") || line.contains("in data.profile_fields")
        })
        .collect();

    assert!(
        raw_loops.is_empty(),
        "iterate `accepted_profile_fields(&data.profile_fields)` instead — the \
         raw map is caller-sized and unbounded. Found: {raw_loops:?}"
    );
    assert!(
        source.matches("accepted_profile_fields(").count() >= 3,
        "expected the accessor's definition plus both consumers (profile \
         upsert and registration responses)"
    );
}

/// `is_profile_field` records whether an answer *also* updated
/// `developer_profiles`. DevRel read that column from the outside to work out
/// where 2,103 rows came from, so it must not be hardcoded true again.
#[test]
fn the_profile_field_flag_is_derived_not_asserted() {
    let source = read(CONTACT);

    assert!(
        source
            .contains("let is_profile_field = !crate::db::developers::is_event_scoped_field(key)"),
        "the flag must be derived from the key's namespace"
    );
    assert!(
        !source.contains("responses.push((key, value, true))")
            && !source.contains("responses.push((key.as_str(), value.as_str(), true))"),
        "an event-scoped answer does not update the profile and must not claim to"
    );
}

/// The event-scoped namespace has to be skipped before the allowlist rejects
/// it, or a question set logs one warning per answer per respondent.
#[test]
fn event_scoped_answers_skip_the_profile_upsert() {
    let source = read(CONTACT);

    let skip = source
        .find("if crate::db::developers::is_event_scoped_field(key)")
        .expect("the profile-upsert loop must skip event-scoped keys");
    let upsert = source
        .find("upsert_developer_field(d1, email, key, value)")
        .expect("the profile upsert call must still exist");

    assert!(
        skip < upsert,
        "the namespace check has to come before the upsert, not after it"
    );
}
