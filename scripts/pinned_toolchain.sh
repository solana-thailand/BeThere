#!/usr/bin/env bash
# Sourced by the release build scripts (worker/deploy.sh,
# frontend-leptos/build.sh) so the wasm they produce is built on the channel in
# rust-toolchain.toml, not on whatever the calling shell exports.
#
# RUSTUP_TOOLCHAIN outranks rust-toolchain.toml. Agent shells on the build host
# export it (1.97.1 on 2026-09-24), which would ship a toolchain CI no longer
# tests. `use_pinned_toolchain` drops it and prints the rustc that will build.

use_pinned_toolchain() {
    if [[ -n "${RUSTUP_TOOLCHAIN:-}" ]]; then
        echo "🦀 Ignoring RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN}; rust-toolchain.toml pins the build."
        unset RUSTUP_TOOLCHAIN
    fi
    echo "🦀 $(rustc --version)"
}
