//! Google Sheets I/O goes through `crate::http`.
//!
//! The sheet modules used to build their own `Headers`/`RequestInit`/`Fetch`
//! for a delete, a batch clear and two copies of a `put_json_ignore` that
//! duplicated `http::put_json`. Each copy had its own status check and its own
//! error text. `http::send_json` now holds the headers, body encoding and
//! status check once; this guard keeps the sheet modules on it.

use std::{fs, path::Path};

const RAW_REQUEST_MARKERS: [&str; 4] = [
    "Fetch::Request",
    "Fetch::Url",
    "RequestInit::new",
    "Headers::new",
];

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|e| panic!("{} is unreadable: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        match path.is_dir() {
            true => rust_files(&path, out),
            false if path.extension().is_some_and(|ext| ext == "rs") => out.push(path),
            false => {}
        }
    }
}

#[test]
fn sheet_modules_do_not_build_their_own_requests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("sheets");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    assert!(
        files.len() > 10,
        "found only {} files under src/sheets — was the module moved?",
        files.len()
    );

    let offenders: Vec<String> = files
        .iter()
        .flat_map(|path| {
            let code = fs::read_to_string(path).expect("readable source");
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .display()
                .to_string();
            code.lines()
                .enumerate()
                .filter(|(_, line)| !line.trim_start().starts_with("//"))
                .filter(|(_, line)| RAW_REQUEST_MARKERS.iter().any(|m| line.contains(m)))
                .map(|(n, line)| format!("{rel}:{}: {}", n + 1, line.trim()))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "sheet modules must send through crate::http (post_json, post_json_status, put_json, \
         get_json), not build requests themselves:\n{}",
        offenders.join("\n")
    );
}
