//! Guard: log fingerprints are keyed by `log_fingerprint_key`, not the session
//! secret (plan 029, ISO 27001 8.11).
//!
//! Every personal identifier in the log stream is a keyed one-way fingerprint
//! (Issue 070). Keying it with `JWT_SECRET` tied it to the session-signing key:
//! the same secret both signs sessions and protects the fingerprints, and
//! rotating one rotates the other. `AppConfig::log_fingerprint_key` is
//! `LOG_FINGERPRINT_KEY`, or `JWT_SECRET` until that secret is provisioned.
//! Source scans, because the key only exists inside the Workers runtime.

fn code_of(relative: &str) -> String {
    let path = format!("{}/src/{relative}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path} must be readable: {e}"))
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn all_sources() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).expect("src must be readable") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = path
                    .strip_prefix(root)
                    .expect("under src")
                    .to_string_lossy()
                    .into_owned();
                out.push((rel.clone(), code_of(&rel)));
            }
        }
    }
    let root = std::path::PathBuf::from(format!("{}/src", env!("CARGO_MANIFEST_DIR")));
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out
}

#[test]
fn no_fingerprint_is_keyed_by_the_session_secret() {
    let sources = all_sources();
    assert!(sources.len() > 100, "the scan must see the worker tree");
    for (path, code) in &sources {
        let squashed: String = code.split_whitespace().collect();
        for banned in [
            "LogRedactor::new(&state.config.jwt_secret)",
            "LogRedactor::new(&self.config.jwt_secret)",
        ] {
            let banned: String = banned.split_whitespace().collect();
            assert!(
                !squashed.contains(&banned),
                "{path} keys a log fingerprint with jwt_secret; use \
                 `state.log_redactor()` / `state.log_fingerprint(..)`"
            );
        }
        for (i, _) in squashed.match_indices("identity_fingerprint(") {
            let call = &squashed[i..squashed[i..].find(')').map_or(squashed.len(), |j| i + j)];
            assert!(
                !call.contains("jwt_secret"),
                "{path} fingerprints with jwt_secret: {call}"
            );
        }
    }
}

#[test]
fn the_state_helpers_use_the_log_key() {
    let state = code_of("state.rs");
    for helper in [
        "identity_fingerprint(identifier, &self.config.log_fingerprint_key)",
        "LogRedactor::new(&self.config.log_fingerprint_key)",
    ] {
        assert!(state.contains(helper), "state.rs must contain `{helper}`");
    }
    assert!(
        state.contains("get_secret(env, \"LOG_FINGERPRINT_KEY\")"),
        "the dedicated secret must be read"
    );
    assert!(
        state.contains("let log_fingerprint_key = log_fingerprint_key(env);")
            && state.lines().any(|l| l.trim() == "log_fingerprint_key,"),
        "AppConfig must take the resolved key"
    );
}
