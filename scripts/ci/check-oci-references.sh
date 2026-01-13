#!/usr/bin/env bash

set -exuo pipefail
cd "$(dirname "$0")/../.."

# Configuration
SOURCE_FILE="obelisk-external.toml"
TARGET_FILES=("obelisk-local-postgres.toml" "obelisk-local.toml" "obelisk-oci.toml")

# Initialize fail flag
EXIT_CODE=0

# Check if source file exists
if [ ! -f "$SOURCE_FILE" ]; then
    echo "Error: Source file '$SOURCE_FILE' not found."
    exit 1
fi

echo "Reading references from $SOURCE_FILE..."

# Extract OCI values using grep and awk
# 1. grep 'location.oci': gets the relevant lines
# 2. awk -F'"': splits by quote and prints the 2nd column (the content inside quotes)
REFERENCES=$(grep 'location.oci' "$SOURCE_FILE" | awk -F'"' '{print $2}')

# Check if we found any references
if [ -z "$REFERENCES" ]; then
    echo "No 'location.oci' entries found in source file."
    exit 0
fi

# Iterate over each OCI reference found in the external config
for OCI_REF in $REFERENCES; do
    echo "---------------------------------------------------"
    echo "Checking: $OCI_REF"

    for TARGET in "${TARGET_FILES[@]}"; do
        if [ ! -f "$TARGET" ]; then
            echo "  [ERROR] File missing: $TARGET"
            EXIT_CODE=1
            continue
        fi

        # grep -F: Matches fixed strings (prevents issues with dots/colons in URLs)
        # -q: Quiet mode (exit 0 if found, 1 if not)
        if grep -Fq "$OCI_REF" "$TARGET"; then
            echo "  [OK] Found in $TARGET"
        else
            echo "  [FAIL] MISSING in $TARGET"
            EXIT_CODE=1
        fi
    done
done

echo "---------------------------------------------------"

if [ $EXIT_CODE -eq 0 ]; then
    echo "SUCCESS: All external OCI references are present in target files."
    exit 0
else
    echo "FAILURE: Some OCI references are missing."
    exit 1
fi

