#!/usr/bin/env bash

# Collect versions of binaries installed by `nix develop` producing file `dev-deps.txt`.
# This script should be executed after every `nix flake update`.

set -exuo pipefail
cd "$(dirname "$0")/.."

rm -f dev-deps.txt
cargo upgrade --version >> dev-deps.txt
cargo-expand --version >> dev-deps.txt
cargo-insta --version >> dev-deps.txt
cargo-nextest --version | head -n 1 >> dev-deps.txt
just --version >> dev-deps.txt
obelisk --version >> dev-deps.txt
echo "pkg-config $(pkg-config --version)" >> dev-deps.txt
rustc --version >> dev-deps.txt
wasm-tools --version >> dev-deps.txt
wasmtime --version >> dev-deps.txt
# libc
ldd --version | head -n 1 >> dev-deps.txt
