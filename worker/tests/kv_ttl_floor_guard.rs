//! `.issues/156` — Cloudflare KV rejects `expiration_ttl` below 60 seconds.
//!
//! The Solana blockhash cache put with a 30 s TTL. Every put failed with
//! `400 Invalid expiration_ttl of 30`, the failure was only logged, and so the
//! cache never held anything and every transaction build called the RPC. It
//! compiled, passed clippy and never failed a request.
//!
//! This guard reads every `.expiration_ttl(…)` argument under `worker/src`. A
//! literal, or a `const` declared anywhere in `worker/src` as a product of
//! literals, must be at least [`KV_MIN_TTL_SECS`]. Any other argument must be
//! on [`CLAMPED_ARGS`] with the reason it cannot go below the floor.
//!
//! ## Run
//!
//! ```sh
//! cargo test -p event-checkin-worker --test kv_ttl_floor_guard
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const WORKER_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Cloudflare KV's minimum `expiration_ttl`, in seconds.
const KV_MIN_TTL_SECS: u64 = 60;

/// Runtime TTL arguments that are clamped to the floor before the put.
/// (file relative to `worker/`, argument text, why it is safe)
const CLAMPED_ARGS: &[(&str, &str, &str)] = &[(
    "src/auth.rs",
    "ttl",
    "`.max(KV_MIN_TTL)` where KV_MIN_TTL = 60",
)];

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}"));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        match path.is_dir() {
            true => collect_rs_files(&path, out),
            false if path.extension().is_some_and(|x| x == "rs") => out.push(path),
            false => {}
        }
    }
}

fn source_files() -> Vec<(String, String)> {
    let root = Path::new(WORKER_ROOT);
    let mut files = Vec::new();
    collect_rs_files(&root.join("src"), &mut files);
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(root)
                .expect("under worker/")
                .to_string_lossy()
                .replace('\\', "/");
            let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
            (rel, text)
        })
        .collect()
}

/// `86_400 * 90` → 7_776_000. `None` if any factor is not an integer literal.
fn eval_product(expr: &str) -> Option<u64> {
    expr.split('*')
        .map(|factor| factor.trim().replace('_', "").parse::<u64>().ok())
        .try_fold(1u64, |acc, factor| factor.map(|f| acc * f))
}

/// Every `const NAME: u64 = <product of literals>;` in the given sources.
fn integer_consts(files: &[(String, String)]) -> HashMap<String, u64> {
    let mut consts = HashMap::new();
    for (_, text) in files {
        for line in text.lines() {
            let Some(rest) = line.trim().split("const ").nth(1) else {
                continue;
            };
            let Some((name, rest)) = rest.split_once(": u64 =") else {
                continue;
            };
            let Some(value) = eval_product(rest.trim().trim_end_matches(';')) else {
                continue;
            };
            consts.insert(name.trim().to_string(), value);
        }
    }
    consts
}

/// The text inside each `.expiration_ttl(…)` call, with its line number.
fn ttl_args(text: &str) -> Vec<(usize, String)> {
    const NEEDLE: &str = ".expiration_ttl(";
    let mut out = Vec::new();
    for (idx, _) in text.match_indices(NEEDLE) {
        let start = idx + NEEDLE.len();
        let Some(len) = text[start..].find(')') else {
            continue;
        };
        let line = text[..idx].matches('\n').count() + 1;
        out.push((line, text[start..start + len].trim().to_string()));
    }
    out
}

fn resolve(arg: &str, consts: &HashMap<String, u64>) -> Option<u64> {
    eval_product(arg).or_else(|| consts.get(arg).copied())
}

#[test]
fn every_kv_ttl_meets_the_60_second_floor() {
    let files = source_files();
    let consts = integer_consts(&files);
    let mut seen = 0;
    let mut failures = Vec::new();

    for (rel, text) in &files {
        for (line, arg) in ttl_args(text) {
            seen += 1;
            let clamped = CLAMPED_ARGS
                .iter()
                .any(|(file, name, _)| *file == rel.as_str() && *name == arg);
            match (resolve(&arg, &consts), clamped) {
                (Some(secs), _) if secs >= KV_MIN_TTL_SECS => {}
                (Some(secs), _) => failures.push(format!(
                    "{rel}:{line}: expiration_ttl({arg}) = {secs}s, KV rejects < {KV_MIN_TTL_SECS}s"
                )),
                (None, true) => {}
                (None, false) => failures.push(format!(
                    "{rel}:{line}: expiration_ttl({arg}) cannot be resolved; use a const or add it to CLAMPED_ARGS with the clamp"
                )),
            }
        }
    }

    assert!(
        seen >= 15,
        "found only {seen} expiration_ttl calls; the scan is broken"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn clamped_args_still_exist() {
    let files = source_files();
    for (file, name, _) in CLAMPED_ARGS {
        let (_, text) = files
            .iter()
            .find(|(rel, _)| rel == file)
            .unwrap_or_else(|| panic!("{file} is gone; drop it from CLAMPED_ARGS"));
        assert!(
            ttl_args(text).iter().any(|(_, arg)| arg == name),
            "{file} no longer calls expiration_ttl({name}); drop it from CLAMPED_ARGS"
        );
    }
}

#[test]
fn scanner_catches_a_sub_floor_ttl() {
    let files = vec![(
        "src/x.rs".to_string(),
        "const SHORT_TTL: u64 = 30;\nconst LONG_TTL: u64 = 7 * 86_400;".to_string(),
    )];
    let consts = integer_consts(&files);
    assert_eq!(consts.get("SHORT_TTL"), Some(&30));
    assert_eq!(consts.get("LONG_TTL"), Some(&604_800));

    let args = ttl_args("kv.put(k, v)?\n    .expiration_ttl(SHORT_TTL)\n    .execute()");
    assert_eq!(args, vec![(2, "SHORT_TTL".to_string())]);
    assert_eq!(resolve(&args[0].1, &consts), Some(30));
    assert_eq!(resolve("300", &consts), Some(300));
    assert_eq!(resolve("ttl", &consts), None);
}
