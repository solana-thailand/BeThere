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

// ---------------------------------------------------------------------------
// The frontend gate (added 2026-09-22, .issues/135)
//
// The worker had a size gate and the frontend had none — and the frontend is
// the bigger of the two: 1.77 MB brotli of first load against a 1.57 MB
// whole-worker bundle, downloaded by every attendee on venue mobile data
// before they can see their ticket. Every invariant here fails silently in
// exactly the way the worker ones do.
// ---------------------------------------------------------------------------

/// Read one `KEY=value` from `frontend-leptos/.size-budget`.
fn frontend_budget(key: &str) -> u64 {
    let file = repo_file("../frontend-leptos/.size-budget");
    let prefix = format!("{key}=");
    let raw = file
        .lines()
        .find_map(|l| l.trim().strip_prefix(&prefix).map(str::trim))
        .unwrap_or_else(|| panic!("frontend-leptos/.size-budget is missing {key}"));
    raw.parse()
        .unwrap_or_else(|e| panic!("frontend .size-budget {key}={raw:?} is not a number: {e}"))
}

#[test]
fn ci_runs_the_frontend_size_gate() {
    let ci = repo_file("../.github/workflows/ci.yml");
    assert!(
        ci.contains("scripts/verify/frontend_size_budget.sh"),
        "CI must run the frontend size gate. The e2e job is the only job that builds the \
         SPA, so it is the only place the frontend can be measured at all — if the step \
         is dropped, nothing measures the larger half of what users download"
    );
}

#[test]
fn the_frontend_gate_cannot_pass_on_a_half_built_bundle() {
    let gate = strip_sh_comments(&repo_file("../scripts/verify/frontend_size_budget.sh"));
    assert!(
        gate.contains("pass vacuously"),
        "the frontend gate must reject a dist that is empty or index.html-only. Without \
         that check it reports a few KiB as comfortably under budget, which is the shape \
         of a green gate that measures nothing (.issues/072)"
    );
    assert!(
        gate.contains("references files that are not in"),
        "an asset referenced by index.html but absent from dist must be an error, not a \
         skip. A reference that silently measures as zero is how a gate stops seeing a \
         whole asset while still reporting green"
    );
}

/// The gate's whole claim is that it measures what an attendee waits for.
/// Cloudflare serves these assets `br` at roughly quality 4 — measured against
/// the deployed site — so judging gzip, or judging brotli at 11, would report
/// a number nobody downloads. brotli-11 understates it by ~28%.
#[test]
fn the_frontend_gate_measures_what_cloudflare_actually_serves() {
    let gate = strip_sh_comments(&repo_file("../scripts/verify/frontend_size_budget.sh"));
    assert!(
        gate.contains("brotliCompressSync"),
        "the frontend gate must measure brotli — Cloudflare serves these assets with \
         content-encoding: br, so a gzip number is not what any attendee waits for"
    );
    assert!(
        gate.contains("BROTLI_PARAM_QUALITY"),
        "the brotli quality must be set explicitly. The default is 11; Cloudflare \
         compresses on the fly at roughly 4, and the gap between them is ~28% of the \
         bundle — large enough that leaving it implicit is a measurement error, not a \
         detail"
    );
}

#[test]
fn frontend_budget_thresholds_are_coherent() {
    let ceiling = frontend_budget("CEILING_BYTES");
    let warn_pct = frontend_budget("WARN_PCT");
    let fail_pct = frontend_budget("FAIL_PCT");
    let baseline = frontend_budget("BASELINE_BYTES");
    let max_growth = frontend_budget("MAX_GROWTH_BYTES");
    let warn_growth = frontend_budget("WARN_GROWTH_BYTES");

    assert!(
        warn_pct < fail_pct && fail_pct <= 100,
        "warn ({warn_pct}%) must sit below fail ({fail_pct}%), and fail must not exceed \
         the budget itself"
    );
    assert!(
        warn_growth < max_growth,
        "the growth warn line ({warn_growth}) must sit below the growth fail line \
         ({max_growth})"
    );
    assert!(
        baseline > 0,
        "BASELINE_BYTES is 0 — the gate would report the entire bundle as growth and be \
         red on a clean tree, which means it will be ignored"
    );
    assert!(
        baseline <= ceiling * warn_pct / 100,
        "BASELINE_BYTES ({baseline}) is already past the warn line — a gate that is amber \
         on a clean tree is a gate nobody reads"
    );
    // The number this gate was built to make loud: .issues/134 option 2 moves a
    // QR + image decoder into the frontend, measured at +257,210 bytes. If the
    // growth allowance ever grows past it, that decision lands silently.
    assert!(
        max_growth < 257_210,
        "MAX_GROWTH_BYTES ({max_growth}) must stay below the 257,210-byte frontend QR \
         decoder in .issues/134 option 2 — that is the specific change this allowance \
         exists to make visible, and an allowance wide enough to swallow it is not one"
    );
}

/// Two gates measuring two bundles will drift; that is what sibling code does.
/// These are the properties where drift would be silent AND harmful, so they
/// are asserted on both scripts at once rather than left to whoever edits one.
#[test]
fn both_size_gates_keep_the_properties_that_make_them_trustworthy() {
    let worker = strip_sh_comments(&repo_file("../scripts/verify/worker_size_budget.sh"));
    let frontend = strip_sh_comments(&repo_file("../scripts/verify/frontend_size_budget.sh"));

    for (name, gate) in [("worker", &worker), ("frontend", &frontend)] {
        // The budget file is data. `source` would execute whatever a bad merge
        // dropped into it, in a script that deploy.sh runs.
        assert!(
            gate.contains("awk -F= -v k="),
            "the {name} gate must PARSE its budget file, not source it"
        );
        assert!(
            !gate.contains("source \"$BUDGET_FILE\"") && !gate.contains(". \"$BUDGET_FILE\""),
            "the {name} gate must never source its budget file"
        );
        // A missing key must stop the gate, not fall back to a default — a
        // budget that silently defaults is a budget nobody is enforcing.
        assert!(
            gate.contains("is missing $key"),
            "the {name} gate must refuse a budget file with a missing key rather than \
             defaulting"
        );
        // Without this the only way to notice growth is to read the number,
        // and nobody reads a number that is always green.
        assert!(
            gate.contains("--update-baseline"),
            "the {name} gate must support --update-baseline, so an intentional bump is a \
             visible line in the same commit as the change that caused it"
        );
    }
}
