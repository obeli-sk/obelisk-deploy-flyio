#!/usr/bin/env bash

set -exuo pipefail
cd "$(dirname "$0")/.."

rm -rf wit/deps/obelisk_*
obelisk generate wit-deps --deployment obelisk-external.toml wit/deps --force
