#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

export PATH="$HOME/.cargo/bin:$PATH"

# Homebrew's rustc can shadow rustup and lacks the wasm target. Pin the
# toolchain used for the Worker build when Rust 1.89 is installed.
if command -v rustup >/dev/null 2>&1 && rustup run 1.89.0 rustc --version >/dev/null 2>&1; then
  export RUSTUP_TOOLCHAIN=1.89.0
  export PATH="$(dirname "$(rustup which rustc)"):$PATH"
fi

if ! command -v worker-build >/dev/null 2>&1; then
  echo "worker-build is required: cargo install worker-build@0.8.0" >&2
  exit 1
fi

exec worker-build --release
