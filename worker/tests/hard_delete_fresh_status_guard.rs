//! Archive then immediate delete used to 400 "must be archived" once, then
//! succeed on a retry: event writes land in D1 first and the delete guard
//! read the eventually consistent KV copy. The guard now reads the D1 copy
//! when there is one. Pinned here because the race needs two edge reads to
//! reproduce and no test here can make KV stale.

const STORE: &str = include_str!("../src/event_store/write/lifecycle.rs");
const HANDLER: &str = include_str!("../src/handlers/events/lifecycle.rs");

fn body_of<'a>(src: &'a str, signature: &str) -> &'a str {
    let start = src
        .find(signature)
        .unwrap_or_else(|| panic!("{signature} missing"));
    let rest = &src[start..];
    &rest[..rest.find("\n}\n").expect("end of fn")]
}

#[test]
fn the_status_guard_prefers_the_fresh_copy() {
    let store = body_of(STORE, "pub async fn hard_delete_event(");
    let pick = store
        .find("let config = fresh.unwrap_or(&kv_config);")
        .expect("the guard must read the D1 copy when given one");
    let guard = store
        .find("if config.status != EventStatus::Archived")
        .expect("archived guard");
    assert!(pick < guard, "choose the copy before checking its status");
}

#[test]
fn the_handler_reads_d1_before_deleting_and_passes_it_down() {
    let handler = body_of(HANDLER, "pub async fn hard_delete_event(");
    let read = handler
        .find("crate::db::events::get_event(d1, &id)")
        .expect("D1 read");
    let call = handler
        .find("crate::event_store::hard_delete_event(kv_ref, &id, force, d1_config.as_ref())")
        .expect("the delete must receive the D1 copy");
    assert!(read < call);
}
