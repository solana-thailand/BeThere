//! .issues/160: a PDPA deletion never reports a bank-slip image erased when
//! the check or the delete failed. Every R2 erase goes through
//! `erase_r2_object`, whose failures are recorded (the reply says `partial`).

const SRC: &str = include_str!("../src/handlers/privacy.rs");

#[test]
fn r2_erases_are_checked_and_failures_recorded() {
    assert!(
        !SRC.contains("let _ = crate::storage::delete"),
        "a dropped delete error"
    );
    assert!(
        !SRC.contains("crate::storage::exists(bucket, &slip_key)\n                        .await\n                        .unwrap_or(false)"),
        "a failed check read as absent"
    );
    assert!(SRC.contains("match erase_r2_object(bucket, &key).await"));
    let failed = SRC.find("R2Erase::Failed(e) => {").expect("failed arm");
    assert!(SRC[failed..failed + 300].contains("summary.record_failure(what)"));
    // the helper: a failed existence check is a failure, not "absent"
    let helper = SRC.find("async fn erase_r2_object").expect("helper");
    assert!(SRC[helper..].contains("Err(e) => R2Erase::Failed(format!(\"{e:?}\")),\n    }\n}"));
}
