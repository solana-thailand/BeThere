//! The deposit promises are written once, in `src/utils/deposit_copy.rs`.
//!
//! On 2026-09-28 the owner decided that a deposit is never forfeited and that
//! THB comes back within 7 days (`docs/deposit-commitment-model.md` §5).
//! Applying that took edits in four surfaces, one of which (the registration
//! checkbox) was nearly missed. A promise copied per page drifts the next time
//! a decision changes, so pin it to one home.

use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => rust_files(&path, out),
            false => {
                if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
    }
}

/// The promises' tell-tale phrases, EN and TH (.plans/037 §2).
const PROMISE_PHRASES: [&str; 4] = ["never forfeited", "within 7 days", "ไม่ถูกริบ", "ภายใน 7 วัน"];

#[test]
fn no_catalog_file_states_the_promises() {
    let locales = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut offenders = Vec::new();
    for locale in std::fs::read_dir(&locales).expect("locales").flatten() {
        for file in std::fs::read_dir(locale.path())
            .expect("locale dir")
            .flatten()
        {
            let text = std::fs::read_to_string(file.path()).expect("catalog file");
            let lower = text.to_lowercase();
            if PROMISE_PHRASES.iter().any(|p| lower.contains(p)) {
                offenders.push(file.path().display().to_string());
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the catalog carries the words around the promise, not the promise:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn only_deposit_copy_states_the_promises() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(files.len() > 20, "the scan must see the frontend sources");

    let mut offenders = Vec::new();
    for path in files {
        if path.ends_with("utils/deposit_copy.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            let lower = line.to_lowercase();
            if PROMISE_PHRASES.iter().any(|p| lower.contains(p)) {
                offenders.push(format!("{}:{}", path.display(), n + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "write the deposit promise with utils::deposit_copy, not inline:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_home_states_the_current_decisions() {
    let home = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/utils/deposit_copy.rs"),
    )
    .expect("deposit_copy.rs");
    assert!(home.contains("\"within 7 days after the event\""));
    assert!(home.contains("\"Your deposit is never forfeited.\""));
    assert!(home.contains("\"ภายใน 7 วันหลังจบงาน\""));
    assert!(home.contains("\"เงินมัดจำของคุณจะไม่ถูกริบ\""));
}
