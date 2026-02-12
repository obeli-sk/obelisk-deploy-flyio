#!/usr/bin/env bash

# Pushes all WASM components to the Docker Hub and updates obelisk-oci.toml

set -exuo pipefail
cd "$(dirname "$0")/.."

TAG="$1"
TOML_FILE="obelisk-oci.toml"
PREFIX="docker.io/getobelisk/components_flyio_"

push() {
    local COMPONENT_TYPE=$1
    local RELATIVE_PATH=$2

    local FILE_NAME_WITHOUT_EXT=$(basename "$RELATIVE_PATH" | sed 's/\.[^.]*$//')
    local OCI_LOCATION="${PREFIX}${FILE_NAME_WITHOUT_EXT}:${TAG}"
    echo "Pushing ${RELATIVE_PATH} to ${OCI_LOCATION}..."
    local OUTPUT=$(obelisk component push "$RELATIVE_PATH" "$OCI_LOCATION")

    # Replace the old location with the actual OCI location

    sed -i -E "/name = \"${FILE_NAME_WITHOUT_EXT}\"/{n;s|location\.oci = \".*\"|location.oci = \"${OUTPUT}\"|}" "$TOML_FILE"
    obelisk component add ${COMPONENT_TYPE} ${OUTPUT} --name ${FILE_NAME_WITHOUT_EXT} -c $TOML_FILE
}

# Build components
just build

push workflow "target/wasm32-unknown-unknown/release_workflow/obelisk_deployer_flyio.wasm"
push webhook_endpoint "target/wasm32-wasip2/release_webhook/webhook_healthcheck.wasm"

echo "All components pushed and TOML file updated successfully."

# obelisk.toml is parsed in toml.rs, snapshots will need updating.
INSTA_UPDATE=always just test
