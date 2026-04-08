#!/usr/bin/env bash

set -exuo pipefail
cd "$(dirname "$0")/.."

obelisk generate wit-deps --deployment obelisk-external.toml wit/deps --overwrite
