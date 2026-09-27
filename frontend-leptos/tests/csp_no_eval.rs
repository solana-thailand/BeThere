//! The CSP grants no `'unsafe-eval'`, so `js_sys::eval` throws in the
//! browser. `/checkin/nfc` started its WebNFC scan through eval and the scan
//! never ran. JS interop goes through wasm_bindgen imports or `Reflect`.

use std::fs;
use std::path::Path;

fn rust_sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        match path.is_dir() {
            true => rust_sources(&path, out),
            false if path.extension().is_some_and(|e| e == "rs") => {
                out.push((
                    path.display().to_string(),
                    fs::read_to_string(&path).unwrap(),
                ));
            }
            false => {}
        }
    }
}

#[test]
fn no_source_calls_js_eval() {
    let mut sources = Vec::new();
    rust_sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut sources,
    );
    assert!(
        sources.len() > 50,
        "walked too few files: {}",
        sources.len()
    );
    let offenders: Vec<&str> = sources
        .iter()
        .filter(|(_, body)| {
            body.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .any(|l| l.contains("js_sys::eval(") || l.contains("Function::new_with_args("))
        })
        .map(|(path, _)| path.as_str())
        .collect();
    assert!(
        offenders.is_empty(),
        "eval under a no-unsafe-eval CSP: {offenders:?}"
    );
}

#[test]
fn nfc_scan_starts_without_eval() {
    let page = include_str!("../src/pages/nfc_checkin.rs");
    assert!(page.contains("fn start_ndef_scan("));
    assert!(page.contains("Reflect::construct("));
}
