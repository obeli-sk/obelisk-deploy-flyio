#!/usr/bin/env bash

# Prints JSON containing arguments to `app-init` function.

OBELISK_VERSION=${OBELISK_VERSION:-$(obelisk -v | cut -d ' ' -f 2)}
SECRETS_DEADLINE_SECS=${SECRETS_DEADLINE_SECS:-120}
HEALTH_CHECK_DEADLINE_SECS=${HEALTH_CHECK_DEADLINE_SECS:-120}
SKIP_CLEANUP=${SKIP_CLEANUP:-false}
MINIO=${MINIO:-true}
VM_STARTUP_DEADLINE_SECS=${VM_STARTUP_DEADLINE_SECS:-30}

cat <<EOF
[
"$FLY_ORG_SLUG",
"$FLY_APP_NAME",
{
    "obelisk_version": "$OBELISK_VERSION",
    "activity_wasm_list":[
        {
            "name": "stargazers_activity_llm_chatgpt",
            "location_oci": "oci://docker.io/getobelisk/demo_stargazers_activity_llm_openai:2025-12-08@sha256:f50464f5bd26e6ebbbe1915f577f04cd67b49f94d2d2c8d5f3b3e8e4fda5b1e5",
            "outbound_secrets":["OPENAI_API_KEY"],
            "lock_expiry_seconds": 10
        },
        {
            "name": "stargazers_activity_github_impl",
            "location_oci": "oci://docker.io/getobelisk/demo_stargazers_activity_github_impl:2025-12-08@sha256:f281f3103883ea3bbc0130f5fdc00ae93eda27cd5a41829dbc1ad56e290478a3",
            "outbound_secrets": ["GITHUB_TOKEN"],
            "lock_expiry_seconds": 5
        },
        {
            "name": "stargazers_activity_db_turso",
            "location_oci": "oci://docker.io/getobelisk/demo_stargazers_activity_db_turso:2025-12-08@sha256:cdad4f289abdc68e1d062f45a717478df2a7c3576940b00644d81bceeea94264",
            "env_vars": ["TURSO_LOCATION"],
            "outbound_secrets": ["TURSO_TOKEN"],
            "lock_expiry_seconds": 5
        }
    ],
    "workflow_list":[
        {
            "name": "stargazers_workflow",
            "location_oci": "oci://docker.io/getobelisk/demo_stargazers_workflow:2025-12-08@sha256:c8a9d14979978564692131f08d912db5fa20f7a8e4253490fae4cbcc7f6286b7"
        }
    ],
    "webhook_endpoint_list":[
        {
            "name": "stargazers_webhook",
            "location_oci": "oci://docker.io/getobelisk/demo_stargazers_webhook:2025-12-08@sha256:1c2a83322fcdf50078e804a8bef7b2ba1e6c56d77285ae2f55e091991bb964ac",
            "routes": [{ "methods": ["POST", "GET"], "route": "" }],
            "exposed_secrets": ["GITHUB_WEBHOOK_SECRET"],
            "secret_exposure_digest":"sha256:125fff622b96971663c05c321ebf52c8b091e234cfb054a438cd29d69b4da518"
        }
    ]
},
{
    "secrets_deadline_secs": $SECRETS_DEADLINE_SECS,
    "health_check_deadline_secs": $HEALTH_CHECK_DEADLINE_SECS,
    "skip_cleanup_on_error": $SKIP_CLEANUP,
    "minio": $MINIO,
    "vm_startup_deadline_secs": $VM_STARTUP_DEADLINE_SECS,
    "expose_api_server": null
}
]
EOF
