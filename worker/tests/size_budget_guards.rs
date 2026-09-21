//! Regression guards for the worker upload-size budget gate.
//!
//! Cloudflare enforces the Worker size limit on the COMPRESSED bundle, and this
//! account is on the free plan: 3 MiB after gzip. Until 2026-09-22 nothing in
//! the repo measured it — `deploy.sh` had no size check and CI's `wasm-build`
//! job built the wasm and discarded the number. Measured that day: 1,557,187
//! bytes gzip, 49.5% of the ceiling.
//!
//! Every invariant below fails SILENTLY if broken. The gate would still be in
//! the tree, the scripts would still be green, and the first sign of trouble
//! would be Cloudflare rejecting a deploy — at which point the change that
//! caused it is already merged and the person deploying is not the person who
//! made it.
//!
//! 1. `deploy.sh` runs the gate, and runs it BEFORE the upload. A gate that
//!    runs after the upload measures a bundle that already shipped.
//! 2. CI runs the gate, so the wall is hit in a pull request rather than in a
//!    deploy.
//! 3. The thresholds are coherent: warn below fail, fail below the hard limit,
//!    and the hard limit is the real free-plan ceiling.
//! 4. The gate cannot pass vacuously on an empty bundle directory (`.issues/072`
//!    — a rule that can only ever pass is not a rule).

use std::fs;
use std::path::Path;

fn repo_file(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Strip `#` comments so a guard cannot be satisfied by prose *about* the thing
/// it is guarding. Mirrors `strip_comments` in `cleanup_guards.rs`, for shell.
fn strip_sh_comments(code: &str) -> String {
    code.lines()
        .map(|l| match l.trim_start().starts_with('#') {
            true => "",
            false => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read one `KEY=value` from `worker/.size-budget`, the same way the shell gate
/// parses it. Panics rather than defaulting: a budget key that silently falls
/// back to a default is a budget nobody is enforcing.
fn budget(key: &str) -> u64 {
    let file = repo_file(".size-budget");
    let prefix = format!("{key}=");
    let raw = file
        .lines()
        .find_map(|l| l.trim().strip_prefix(&prefix).map(str::trim))
        .unwrap_or_else(|| panic!(".size-budget is missing {key}"));
    raw.parse()
        .unwrap_or_else(|e| panic!(".size-budget {key}={raw:?} is not a number: {e}"))
}

#[test]
fn deploy_runs_the_size_gate_before_uploading() {
    let code = strip_sh_comments(&repo_file("deploy.sh"));

    let gate_at = code
        .find("run_size_budget_gate")
        .expect("deploy.sh must run the size budget gate — an unmeasured deploy is the defect");

    // The first real upload. `--dry-run` deploys upload nothing, so they are not
    // the line the gate has to precede; find the first `wrangler deploy` that is
    // NOT a dry run.
    let upload_at = code
        .match_indices("npx wrangler deploy")
        .find(|(idx, _)| {
            let line_end = code[*idx..].find('\n').map_or(code.len(), |n| idx + n);
            !code[*idx..line_end].contains("--dry-run")
        })
        .map(|(idx, _)| idx)
        .expect("deploy.sh must contain a real (non-dry-run) wrangler deploy");

    assert!(
        gate_at < upload_at,
        "the size gate is invoked at byte {gate_at} but the upload starts at byte {upload_at} — \
         a gate that runs after the upload is measuring a bundle that already shipped"
    );
}

#[test]
fn deploy_aborts_when_the_size_gate_fails() {
    let code = strip_sh_comments(&repo_file("deploy.sh"));
    assert!(
        code.contains("if ! run_size_budget_gate; then"),
        "deploy.sh must ABORT on a failed size gate, not merely log it — the nightly \
         cleanup shipped for four days writing its failures only to tracing (commit 3973792)"
    );
}

#[test]
fn ci_runs_the_size_gate() {
    let ci = repo_file("../.github/workflows/ci.yml");
    assert!(
        ci.contains("scripts/verify/worker_size_budget.sh"),
        "CI must run the size budget gate, so a bundle regression reds a pull request \
         instead of surfacing as a rejected deploy after the change is already merged"
    );
}

#[test]
fn budget_thresholds_are_coherent() {
    let ceiling = budget("CEILING_BYTES");
    let warn_pct = budget("WARN_PCT");
    let fail_pct = budget("FAIL_PCT");
    let baseline = budget("BASELINE_BYTES");

    assert_eq!(
        ceiling,
        3 * 1024 * 1024,
        "CEILING_BYTES must be the Cloudflare free-plan Worker limit after gzip (3 MiB). \
         Raising it to the paid-plan 10 MiB is a billing decision, not a code change."
    );
    assert!(
        warn_pct < fail_pct,
        "WARN_PCT ({warn_pct}) must be below FAIL_PCT ({fail_pct}) — otherwise the warning \
         can never fire and the first signal is a hard failure"
    );
    assert!(
        fail_pct < 100,
        "FAIL_PCT ({fail_pct}) must leave headroom below the hard limit, so the ceiling is \
         discovered by a red build and not by a rejected deploy during an incident"
    );
    assert!(
        baseline < ceiling * fail_pct / 100,
        "BASELINE_BYTES ({baseline}) is already past the fail line — the gate is red on \
         a clean tree, which means it will be ignored"
    );
}

#[test]
fn the_gate_cannot_pass_on_an_empty_bundle() {
    let gate = strip_sh_comments(&repo_file("../scripts/verify/worker_size_budget.sh"));
    assert!(
        gate.contains("pass vacuously"),
        "the size gate must reject an empty bundle directory. Without that check it reports \
         a 0-byte bundle as comfortably under budget, which is the shape of a green gate \
         that measures nothing (.issues/072)"
    );
    assert!(
        gate.contains("! -name '*.map'"),
        "source maps must be excluded from the measurement — wrangler does not upload them \
         (upload_source_maps is unset), so counting them both inflates the number and makes \
         dropping a map look like a size win"
    );
}
