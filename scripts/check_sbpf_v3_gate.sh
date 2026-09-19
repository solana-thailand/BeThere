#!/usr/bin/env bash
# Is the SBPFv3 door open yet?
#
# `.issues/123` says the escrow is SBPFv0 and that SIMD-0500 will one day refuse
# to deploy it. What that issue could not say, because it was read from a blog
# post rather than from a cluster, is that the *other* half of the change has not
# landed either: the feature gate that ENABLES deploying and executing SBPFv3 is
# inactive on mainnet, devnet and testnet. Until it flips, a v3 build is an
# artifact no cluster will take, and upgrading the local toolchain buys nothing
# while risking the 3.1.10 pipeline every escrow script runs on.
#
# So this is the trigger, not the migration. Run it (or cron it); when it says
# ACTIVATED, `.issues/123`'s plan becomes the next thing to do.
#
# Exit codes: 0 still closed · 10 open somewhere · 1 could not tell.
#
# The last one matters more than it looks. A grep that finds nothing reads
# exactly like "not activated yet", so a renamed or dropped feature key would
# report safe forever. Missing key => exit 1, loudly.

set -euo pipefail

# SIMD-0178/0179/0189 — "Enable deployment and execution of SBPFv3 programs".
GATE="BUwGLeF3Lxyfv1J1wY8biFHBB2hrk2QhbNftQf3VV3cC"

command -v solana >/dev/null 2>&1 || {
    echo "solana CLI not on PATH — cannot read the gate" >&2
    exit 1
}

status=0
for cluster in m d t; do
    case "$cluster" in
        m) label="mainnet" ;;
        d) label="devnet" ;;
        t) label="testnet" ;;
        *) label="$cluster" ;;
    esac

    line=$(solana feature status -u "$cluster" 2>/dev/null | grep -F "$GATE" || true)

    if [ -z "$line" ]; then
        echo "$label: FEATURE KEY NOT FOUND — the CLI no longer lists $GATE."
        echo "        Do not read this as 'still closed'. Check the key against"
        echo "        the SIMD and this CLI's version before trusting it again."
        status=1
        continue
    fi

    if printf '%s' "$line" | grep -q "inactive"; then
        echo "$label: closed (SBPFv3 deployment still disabled)"
    else
        echo "$label: ACTIVATED — $(printf '%s' "$line" | cut -d'|' -f2 | xargs)"
        echo "        SBPFv3 is deployable here. .issues/123 is now live work:"
        echo "        toolchain to platform-tools >= v1.56 / cargo-build-sbf >= v4.2.0,"
        echo "        rebuild with --arch v3, verify e_flags == 3, re-run the SVM tests."
        [ "$status" -eq 1 ] || status=10
    fi
done

exit "$status"
