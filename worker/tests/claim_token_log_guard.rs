//! Capability-bearing claim tokens must never cross into application logs.

use std::{fs, path::Path};

fn rust_sources(path: &Path, output: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).expect("source directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            rust_sources(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

#[test]
fn claim_tokens_are_fingerprinted_before_logging() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&source_root, &mut sources);

    for path in sources {
        let source = fs::read_to_string(&path).expect("Rust source is UTF-8");
        assert!(
            !source.contains("claim_token = %") && !source.contains("token = %"),
            "raw claim-token tracing field in {}",
            path.display()
        );
        assert!(
            !source.contains("claim token {token}"),
            "raw claim token interpolated into log message in {}",
            path.display()
        );
    }
}
