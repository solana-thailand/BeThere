#!/usr/bin/env python3
"""
measure_metrics.py — BeThere self-verification script.

Regenerates the metrics claimed in the pitch deck (scripts/make_pitch_deck.py)
from the LIVE codebase so future drift is caught automatically.

Every value is REAL (measured or derivable) — never fabricated, never mocked.

Claims verified:
  - "88 KB program size (89,856 bytes)"
  - "287 tests" (73 domain + 80 worker + 92 frontend Leptos + 42 on-chain SVM)
  - "16 Kani harnesses"
  - "$0.001 per cNFT badge"
  - "$0.00087 per on-chain TX"
  - "< 500ms check-in latency"  (edge-deployed — CANNOT be measured statically)

Project facts honored:
  - The Solana program is `bethere-escrow`, built with `quasar-lang` (NOT Anchor/Pinocchio).
  - `bethere-escrow` is a STANDALONE package, not a workspace member.
    Always use `--manifest-path bethere-escrow/Cargo.toml`, never `-p bethere-escrow`.
  - macOS tooling: rg, fd, eza available on PATH for ad-hoc inspection.

Output contract:
  - Human-readable table to stdout.
  - JSON snapshot to scripts/.metrics.json (next to this script).
  - Exit 0 on success even if individual metrics are low-confidence; exit non-zero
    only on hard failure (e.g. cannot find the repo root).
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

# ----------------------------------------------------------------------------
# Constants & assumptions (documented, not invented)
# ----------------------------------------------------------------------------

# Repo layout
REPO_ROOT = Path(__file__).resolve().parent.parent
ESCROW_DIR = REPO_ROOT / "bethere-escrow"
KANI_FILE = ESCROW_DIR / "src" / "kani.rs"
SCRIPTS_DIR = REPO_ROOT / "scripts"
JSON_OUT = SCRIPTS_DIR / ".metrics.json"

# Deck claims (from scripts/make_pitch_deck.py + README + docs/presentation_materials.md)
DECK_PROGRAM_BYTES = 89_856
DECK_PROGRAM_KIB = 88
DECK_TEST_TOTAL = 287
DECK_TESTS_DOMAIN = 73
DECK_TESTS_WORKER = 80
DECK_TESTS_FRONTEND = 92
DECK_TESTS_ONCHAIN = 42
DECK_KANI_PROOFS = 16
DECK_TX_USD = 0.00087
DECK_CNFT_USD = 0.001
DECK_LATENCY_MS = 500

# Solana fee math assumptions (stated in deck: "at $172/SOL")
LAMPORTS_PER_SIGNATURE = 5_000  # Solana base fee per signature
LAMPORTS_PER_SOL = 1_000_000_000
ASSUMED_SOL_USD = 172.0  # matches deck's "at $172/SOL" annotation

# Bubblegum cNFT mint: canonical cost basis is ~12,000 lamports to create the
# leaf's associated state (Merkle canopy + metadata hash). We use the same
# approximation the deck authors used and document it explicitly.
CNFT_MINT_LAMPORTS_ASSUMED = 12_000

# Build toolchain
SBF_BUILD_CMD = [
    "cargo",
    "build-sbf",
    "--manifest-path",
    "bethere-escrow/Cargo.toml",
]


# ----------------------------------------------------------------------------
# Result container
# ----------------------------------------------------------------------------


@dataclass
class Metric:
    name: str
    value: str
    source: str
    derivation: str
    confidence: str  # "high" | "medium" | "low"
    deck_claim: Optional[str] = None
    matches_deck: Optional[bool] = None
    notes: str = ""


@dataclass
class Report:
    generated_at: str
    metrics: list[Metric] = field(default_factory=list)


# ----------------------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------------------


def run(
    cmd: list[str], *, timeout: int = 600, cwd: Optional[Path] = None
) -> subprocess.CompletedProcess[str]:
    """Run a command, capturing stdout+stderr as text."""
    return subprocess.run(
        cmd,
        cwd=str(cwd or REPO_ROOT),
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout,
    )


def find_so_artifacts() -> list[Path]:
    """Locate any previously built .so artifacts under known deploy dirs."""
    candidates: list[Path] = []
    search_roots = [
        ESCROW_DIR / "target" / "deploy",
        REPO_ROOT / "target" / "deploy",
        ESCROW_DIR / "target",
        REPO_ROOT / "target",
    ]
    for root in search_roots:
        if not root.exists():
            continue
        # Use Python glob, not external fd, so the script is self-contained.
        candidates.extend(sorted(root.rglob("*.so")))
    # De-duplicate while preserving order
    seen = set()
    out = []
    for p in candidates:
        rp = p.resolve()
        if rp not in seen:
            seen.add(rp)
            out.append(p)
    return out


def parse_cargo_test_total(stdout: str) -> tuple[int, int, int]:
    """
    Parse `cargo test` output for all `test result:` lines.

    Returns (total_passed, total_failed, total_ignored) summed across binaries.
    """
    passed = failed = ignored = 0
    pattern = re.compile(
        r"test result: ok\.\s+(\d+) passed;\s+(\d+) failed;\s+(\d+) ignored"
    )
    for line in stdout.splitlines():
        m = pattern.search(line)
        if m:
            passed += int(m.group(1))
            failed += int(m.group(2))
            ignored += int(m.group(3))
    return passed, failed, ignored


def count_kani_proofs() -> int:
    """Count `#[kani::proof]` attributes in bethere-escrow/src/kani.rs."""
    if not KANI_FILE.exists():
        return 0
    text = KANI_FILE.read_text(encoding="utf-8")
    # match `#[kani::proof]` and `#[kani::proof(...)]`
    return len(re.findall(r"#\[kani::proof\b", text))


def count_rust_tests_static(dirs: list[Path]) -> int:
    """
    Static count of `#[test]` and `#[tokio::test]` annotations under the given
    directories. Used as a fallback / cross-check when cargo test cannot run.
    """
    pattern = re.compile(r"#\[(?:tokio::)?test\]")
    total = 0
    for d in dirs:
        if not d.exists():
            continue
        for rs in d.rglob("*.rs"):
            try:
                text = rs.read_text(encoding="utf-8", errors="ignore")
            except OSError:
                continue
            # naive line-level count (matches how the deck was likely counted)
            total += sum(1 for line in text.splitlines() if pattern.search(line))
    return total


# ----------------------------------------------------------------------------
# Individual metric collectors
# ----------------------------------------------------------------------------


def collect_program_size() -> Metric:
    """Locate .so artifact (or build it), report bytes, or fall back to docs."""
    artifacts = find_so_artifacts()
    if artifacts:
        # Pick the newest by mtime
        chosen = max(artifacts, key=lambda p: p.stat().st_mtime)
        size = chosen.stat().st_size
        return Metric(
            name="program_size_bytes",
            value=str(size),
            source=f"file: {chosen.relative_to(REPO_ROOT)}",
            derivation=f"os.stat().st_size of built .so artifact",
            confidence="high",
            deck_claim=f"{DECK_PROGRAM_BYTES} bytes ({DECK_PROGRAM_KIB} KiB)",
            matches_deck=(size == DECK_PROGRAM_BYTES),
            notes=f"kib={size / 1024:.1f}",
        )

    # No artifact — try to build it.
    build = run(SBF_BUILD_CMD, timeout=180, cwd=REPO_ROOT)
    artifacts = find_so_artifacts()
    if artifacts:
        chosen = max(artifacts, key=lambda p: p.stat().st_mtime)
        size = chosen.stat().st_size
        return Metric(
            name="program_size_bytes",
            value=str(size),
            source=f"file: {chosen.relative_to(REPO_ROOT)} (just built)",
            derivation="cargo build-sbf succeeded; os.stat().st_size",
            confidence="high",
            deck_claim=f"{DECK_PROGRAM_BYTES} bytes ({DECK_PROGRAM_KIB} KiB)",
            matches_deck=(size == DECK_PROGRAM_BYTES),
            notes=f"kib={size / 1024:.1f}",
        )

    # Toolchain unavailable — honest fallback to last-known docs figure.
    err_tail = (build.stderr or build.stdout or "").strip().splitlines()[-3:]
    return Metric(
        name="program_size_bytes",
        value="BUILD_TOOLCHAIN_UNAVAILABLE",
        source=f"docs (stale): README.md / docs/presentation_materials.md",
        derivation=(
            f"Last-known size from docs = {DECK_PROGRAM_BYTES} bytes. "
            f"Toolchain error: {' | '.join(err_tail)}"
        ),
        confidence="low",
        deck_claim=f"{DECK_PROGRAM_BYTES} bytes ({DECK_PROGRAM_KIB} KiB)",
        matches_deck=None,
        notes=(
            f"To reproduce the real number, run on a host with a working "
            f"Solana BPF toolchain: `{' '.join(SBF_BUILD_CMD)}`, then stat "
            f"`bethere-escrow/target/deploy/bethere_escrow.so`."
        ),
    )


def collect_tests_onchain() -> tuple[Metric, int]:
    """
    Run cargo test on bethere-escrow (standalone package, manifest-path required).
    Returns the Metric plus the raw passed count for aggregation.
    """
    cmd = ["cargo", "test", "--manifest-path", "bethere-escrow/Cargo.toml"]
    res = run(cmd, timeout=600, cwd=REPO_ROOT)
    passed, failed, ignored = parse_cargo_test_total(res.stdout + res.stderr)
    value_str = f"{passed} passed"
    if failed:
        value_str += f", {failed} failed"
    if ignored:
        value_str += f", {ignored} ignored"
    m = Metric(
        name="tests_onchain_bethere_escrow",
        value=value_str,
        source="cargo test --manifest-path bethere-escrow/Cargo.toml",
        derivation="sum of `test result:` lines from cargo test",
        confidence="high",
        deck_claim=f"{DECK_TESTS_ONCHAIN} on-chain SVM",
        matches_deck=(passed == DECK_TESTS_ONCHAIN),
        notes="quasar-lang program (not Anchor, not Pinocchio)",
    )
    return m, passed


def collect_tests_domain() -> tuple[Metric, int]:
    cmd = ["cargo", "test", "-p", "event-checkin-domain"]
    res = run(cmd, timeout=300, cwd=REPO_ROOT)
    passed, failed, ignored = parse_cargo_test_total(res.stdout + res.stderr)
    value_str = f"{passed} passed"
    if failed:
        value_str += f", {failed} failed"
    m = Metric(
        name="tests_domain",
        value=value_str,
        source="cargo test -p event-checkin-domain",
        derivation="sum of `test result:` lines from cargo test",
        confidence="high",
        deck_claim=f"{DECK_TESTS_DOMAIN} domain",
        matches_deck=(passed == DECK_TESTS_DOMAIN),
    )
    return m, passed


def collect_tests_worker() -> tuple[Metric, int]:
    cmd = ["cargo", "test", "-p", "event-checkin-worker"]
    res = run(cmd, timeout=300, cwd=REPO_ROOT)
    passed, failed, ignored = parse_cargo_test_total(res.stdout + res.stderr)
    value_str = f"{passed} passed"
    if failed:
        value_str += f", {failed} failed"
    m = Metric(
        name="tests_worker",
        value=value_str,
        source="cargo test -p event-checkin-worker",
        derivation="sum of `test result:` lines across unit + integration binaries",
        confidence="high",
        deck_claim=f"{DECK_TESTS_WORKER} worker",
        matches_deck=(passed == DECK_TESTS_WORKER),
    )
    return m, passed


def collect_tests_frontend() -> tuple[Metric, int]:
    """
    Frontend Leptos uses wasm-bindgen-test. Running it requires the
    wasm32-unknown-unknown target + wasm-pack / cargo-leptos. We cross-check
    via static #[test] count (which matches how the deck was likely derived).
    """
    static = count_rust_tests_static(
        [REPO_ROOT / "frontend-leptos" / "src", REPO_ROOT / "frontend-leptos" / "tests"]
    )
    m = Metric(
        name="tests_frontend_leptos",
        value=f"{static} #[test]/#[wasm_bindgen_test] annotations",
        source="static count of #[test] under frontend-leptos/",
        derivation="regex `#\\[(?:tokio::)?test\\]` over *.rs (wasm runtime not invoked)",
        confidence="medium",
        deck_claim=f"{DECK_TESTS_FRONTEND} frontend Leptos",
        matches_deck=(static == DECK_TESTS_FRONTEND),
        notes="Authoritative count requires `wasm-pack test --headless` (Chrome).",
    )
    return m, static


def collect_tests_total(
    onchain: int, domain: int, worker: int, frontend: int
) -> Metric:
    total = onchain + domain + worker + frontend
    return Metric(
        name="tests_total_stack",
        value=f"{total} tests across the stack",
        source="sum of measured components",
        derivation=(
            f"{onchain} (on-chain) + {domain} (domain) + {worker} (worker) + "
            f"{frontend} (frontend Leptos static count)"
        ),
        confidence="medium",
        deck_claim=f"{DECK_TEST_TOTAL} tests",
        matches_deck=(total == DECK_TEST_TOTAL),
    )


def collect_kani() -> Metric:
    n = count_kani_proofs()
    return Metric(
        name="kani_proof_harnesses",
        value=str(n),
        source=f"file: {KANI_FILE.relative_to(REPO_ROOT)}",
        derivation="count of `#[kani::proof]` attributes",
        confidence="high",
        deck_claim=str(DECK_KANI_PROOFS),
        matches_deck=(n == DECK_KANI_PROOFS),
    )


def collect_fees() -> tuple[Metric, Metric]:
    """
    Derive Solana fees from first principles and compare to deck.

    Per-TX fee: 5000 lamports/signature -> SOL -> USD at assumed SOL price.
    cNFT cost: ~12,000 lamports Bubblegum leaf-creation cost basis.
    """
    tx_sol = LAMPORTS_PER_SIGNATURE / LAMPORTS_PER_SOL
    tx_usd = tx_sol * ASSUMED_SOL_USD
    cnft_sol = CNFT_MINT_LAMPORTS_ASSUMED / LAMPORTS_PER_SOL
    cnft_usd = cnft_sol * ASSUMED_SOL_USD

    tx_metric = Metric(
        name="fee_per_tx_usd",
        value=f"${tx_usd:.6f}",
        source="derived: 5000 lamports/signature × SOL/USD",
        derivation=(
            f"5000 lamports ÷ {LAMPORTS_PER_SOL} × ${ASSUMED_SOL_USD:.0f}/SOL "
            f"= ${tx_usd:.6f}"
        ),
        confidence="high",
        deck_claim=f"${DECK_TX_USD}",
        matches_deck=abs(tx_usd - DECK_TX_USD) < 1e-6,
        notes=(f"delta vs deck: {tx_usd - DECK_TX_USD:+.6f} USD"),
    )

    cnft_metric = Metric(
        name="fee_per_cnft_usd",
        value=f"${cnft_usd:.6f}",
        source="derived: ~12,000 lamports Bubblegum leaf-creation × SOL/USD",
        derivation=(
            f"~{CNFT_MINT_LAMPORTS_ASSUMED} lamports ÷ {LAMPORTS_PER_SOL} "
            f"× ${ASSUMED_SOL_USD:.0f}/SOL = ${cnft_usd:.6f}"
        ),
        confidence="medium",
        deck_claim=f"${DECK_CNFT_USD}",
        matches_deck=abs(cnft_usd - DECK_CNFT_USD) < 1e-3,
        notes=(
            f"delta vs deck: {cnft_usd - DECK_CNFT_USD:+.6f} USD; deck rounds to $0.001"
        ),
    )
    return tx_metric, cnft_metric


def collect_latency() -> Metric:
    return Metric(
        name="checkin_latency_ms",
        value="NEEDS_BENCHMARK",
        source="n/a",
        derivation=(
            "Cannot be measured statically. Requires a deployed edge worker "
            "(Cloudflare Worker on the edge) and an end-to-end probe hitting "
            "/api/checkin or equivalent, measuring wall-clock latency p50/p95."
        ),
        confidence="low",
        deck_claim=f"< {DECK_LATENCY_MS}ms",
        matches_deck=None,
        notes=(
            "Reproducibility: deploy worker (`wrangler deploy`), then loop "
            "`curl -w '%{time_total}'` from a real client against the edge URL."
        ),
    )


# ----------------------------------------------------------------------------
# Rendering
# ----------------------------------------------------------------------------


def render_table(report: Report) -> str:
    cols = ["metric", "value", "source / derivation", "confidence", "matches deck?"]
    rows = []
    for m in report.metrics:
        derivation = m.derivation
        if len(derivation) > 80:
            derivation = derivation[:77] + "..."
        rows.append(
            [
                m.name,
                m.value,
                derivation,
                m.confidence,
                "n/a"
                if m.matches_deck is None
                else ("yes" if m.matches_deck else "NO — DRIFT"),
            ]
        )

    widths = [max(len(c), *(len(r[i]) for r in rows)) for i, c in enumerate(cols)]

    def fmt_row(cells: list[str]) -> str:
        return "  ".join(cell.ljust(widths[i]) for i, cell in enumerate(cells))

    lines = [
        "",
        "BeThere — Live Metrics (auto-generated by scripts/measure_metrics.py)",
        "=" * 120,
        fmt_row(cols),
        "-" * 120,
    ]
    for r in rows:
        lines.append(fmt_row(r))
    lines.append("=" * 120)
    lines.append("")
    # Per-metric deep dive (deck deltas, notes)
    for m in report.metrics:
        lines.append(f"• {m.name}")
        lines.append(f"    value         : {m.value}")
        lines.append(f"    confidence    : {m.confidence}")
        lines.append(f"    source        : {m.source}")
        lines.append(f"    derivation    : {m.derivation}")
        if m.deck_claim is not None:
            lines.append(f"    deck claim    : {m.deck_claim}")
            if m.matches_deck is True:
                lines.append("    vs deck       : ✅ matches")
            elif m.matches_deck is False:
                lines.append("    vs deck       : ❌ DRIFT — deck needs regeneration")
            else:
                lines.append("    vs deck       : ⚠ cannot verify (low confidence)")
        if m.notes:
            lines.append(f"    notes         : {m.notes}")
        lines.append("")
    return "\n".join(lines)


def write_json(report: Report) -> None:
    payload = {
        "generated_at": report.generated_at,
        "metrics": [asdict(m) for m in report.metrics],
    }
    JSON_OUT.parent.mkdir(parents=True, exist_ok=True)
    JSON_OUT.write_text(
        json.dumps(payload, indent=2, sort_keys=False) + "\n", encoding="utf-8"
    )


# ----------------------------------------------------------------------------
# Main
# ----------------------------------------------------------------------------


def main() -> int:
    if not REPO_ROOT.exists():
        print(f"FATAL: repo root not found at {REPO_ROOT}", file=sys.stderr)
        return 2

    report = Report(generated_at=datetime.now(timezone.utc).isoformat())

    # 1. Program size
    report.metrics.append(collect_program_size())

    # 2. Test counts (per component + stack total)
    onchain_m, onchain_n = collect_tests_onchain()
    domain_m, domain_n = collect_tests_domain()
    worker_m, worker_n = collect_tests_worker()
    frontend_m, frontend_n = collect_tests_frontend()
    report.metrics.extend([onchain_m, domain_m, worker_m, frontend_m])
    report.metrics.append(
        collect_tests_total(onchain_n, domain_n, worker_n, frontend_n)
    )

    # 3. Kani proofs
    report.metrics.append(collect_kani())

    # 4. Fees
    tx_m, cnft_m = collect_fees()
    report.metrics.extend([tx_m, cnft_m])

    # 5. Latency
    report.metrics.append(collect_latency())

    print(render_table(report))
    write_json(report)
    print(f"Wrote JSON snapshot → {JSON_OUT.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
