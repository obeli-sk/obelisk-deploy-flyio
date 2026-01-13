#!/usr/bin/env bash

set -exuo pipefail
cd "$(dirname "$0")/.."

# Prints JSON containing arguments to `app-init` function.

OBELISK_VERSION=${OBELISK_VERSION:-$(obelisk -v | cut -d ' ' -f 2)}
SECRETS_DEADLINE_SECS=${SECRETS_DEADLINE_SECS:-120}
HEALTH_CHECK_DEADLINE_SECS=${HEALTH_CHECK_DEADLINE_SECS:-120}
SKIP_CLEANUP=${SKIP_CLEANUP:-false}
MINIO=${MINIO:-true}
VM_STARTUP_DEADLINE_SECS=${VM_STARTUP_DEADLINE_SECS:-30}

ACTIVITY_FLY_OCI=$(awk '/name *= *"activity_fly_http"/ {getline; match($0, /"([^"]+)"/, a); print a[1]}' obelisk-oci.toml)
ACTIVITY_HTTP_OCI=$(awk '/name *= *"activity_http_generic"/ {getline; match($0, /"([^"]+)"/, a); print a[1]}' obelisk-oci.toml)
ACTIVITY_OBELISK_CLIENT_OCI=$(awk '/name *= *"activity_obelisk_client"/ {getline; match($0, /"([^"]+)"/, a); print a[1]}' obelisk-oci.toml)
WORKFLOW_OCI=$(awk '/name *= *"obelisk_deployer_flyio"/ {getline; match($0, /"([^"]+)"/, a); print a[1]}' obelisk-oci.toml)

cat <<EOF
[
"$FLY_ORG_SLUG",
"$FLY_APP_NAME",
{
    "obelisk-version": "$OBELISK_VERSION",
    "activity-wasm-list":[
        {
            "name": "activity_fly_http",
            "location-oci": "$ACTIVITY_FLY_OCI",
            "env-vars":["FLY_API_TOKEN"],
            "lock-expiry-seconds": 15,
            "max-retries": 6
        },
        {
            "name": "activity_http_generic",
            "location-oci": "$ACTIVITY_HTTP_OCI",
            "lock-expiry-seconds": 5
        },
        {
            "name": "activity_obelisk_client",
            "location-oci": "$ACTIVITY_OBELISK_CLIENT_OCI"
        }
    ],
    "workflow-list":[
        {
            "name": "obelisk_deployer_flyio",
            "location-oci": "$WORKFLOW_OCI"
        }
    ]
},
{
    "secrets-deadline-secs": $SECRETS_DEADLINE_SECS,
    "health-check-deadline-secs": $HEALTH_CHECK_DEADLINE_SECS,
    "skip-cleanup-on-error": $SKIP_CLEANUP,
    "minio": $MINIO,
    "vm-startup-deadline-secs": $VM_STARTUP_DEADLINE_SECS,
    "expose-api-server": null
}
]
EOF
