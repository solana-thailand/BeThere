//! The deposit promises are written once, in `src/utils/deposit_copy.rs`.
//!
//! On 2026-09-28 the owner decided that a no-show still gets the deposit back
//! and that THB comes back within 7 days (`docs/deposit-commitment-model.md`
//! §5). On 2026-10-08 the owner retired the "never forfeited" wording of D1.
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
const PROMISE_PHRASES: [&str; 4] = [
    "still get your deposit back",
    "within 7 days",
    "ยังได้เงินมัดจำคืน",
    "ภายใน 7 วัน",
];

/// D1's old wording, retired by the owner on 2026-10-08. No page, catalog
/// entry or promise constant may say it again, in either language.
const RETIRED_PHRASES: [&str; 2] = ["never forfeited", "ไม่ถูกริบ"];

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
    assert!(home.contains("\"You still get your deposit back.\""));
    assert!(home.contains("\"ภายใน 7 วันหลังจบงาน\""));
    assert!(home.contains("\"คุณยังได้เงินมัดจำคืน\""));
}

#[test]
fn the_retired_wording_stays_retired() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    for locale in std::fs::read_dir(root.join("locales"))
        .expect("locales")
        .flatten()
    {
        files.extend(
            std::fs::read_dir(locale.path())
                .expect("locale dir")
                .flatten()
                .map(|f| f.path()),
        );
    }
    assert!(files.len() > 20, "the scan must see the frontend sources");

    let mut offenders = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            let lower = line.to_lowercase();
            if RETIRED_PHRASES.iter().any(|p| lower.contains(p)) {
                offenders.push(format!("{}:{}", path.display(), n + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "D1 is worded as what the attendee gets (owner, 2026-10-08):\n{}",
        offenders.join("\n")
    );
}
