#!/usr/bin/env bash

# Prints JSON containing arguments to `app-init` function.

OBELISK_VERSION=${OBELISK_VERSION:-$(obelisk -v | cut -d ' ' -f 2)}
SECRETS_DEADLINE_SECS=${SECRETS_DEADLINE_SECS:-120}
HEALTH_CHECK_DEADLINE_SECS=${HEALTH_CHECK_DEADLINE_SECS:-120}
SKIP_CLEANUP=${SKIP_CLEANUP:-false}
MINIO=${MINIO:-true}

cat <<EOF
[
"$FLY_ORG_SLUG",
"$FLY_APP_NAME",
"$OBELISK_VERSION",
{
    "activity-wasm-list":[
        {
            "name": "activity_fly_http",
            "location-oci": "docker.io/getobelisk/components_flyio_activity_fly_http:2025-10-05@sha256:fc19825d246fae8110f0473a505bf1da727086832bb0820653ec36e940822024",
            "env-vars":["FLY_API_TOKEN"],
            "lock-expiry-seconds": 15,
            "max-retries": 6
        },
        {
            "name": "http_activity",
            "location-oci": "docker.io/getobelisk/test_programs_http_get_activity:2025-09-28@sha256:8131d9cafbdf06dbaf7a3b4e629791c9cf3dc1553df9418b34f25ab900b72929",
            "lock-expiry-seconds": 5
        }
    ],
    "workflow-list":[
        {
            "name": "obelisk_deployer_flyio",
            "location-oci": "docker.io/getobelisk/components_flyio_obelisk_deployer_flyio:2025-10-05-1@sha256:f2aacd7d90e2b8349d63a6c85f79904e99cfc754210250c24c356cb315089349"
        }
    ]
},
$SECRETS_DEADLINE_SECS,
$HEALTH_CHECK_DEADLINE_SECS,
$SKIP_CLEANUP,
$MINIO
]
EOF
