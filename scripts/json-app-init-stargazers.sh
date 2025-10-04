#!/usr/bin/env bash

# Prints JSON containing arguments to `app-init` function.

SKIP_CLEANUP=${SKIP_CLEANUP:-false}

cat <<EOF
[
"$FLY_ORG_SLUG",
"$FLY_APP_NAME",
{
    "activity-wasm-list":[
        {
            "name": "stargazers_activity_llm_chatgpt",
            "location-oci": "docker.io/getobelisk/demo_stargazers_activity_llm_openai:2025-09-28@sha256:4b10a66c80bec625a6b0a2e8a4b5192f8a2356eca19c0a6705335771a8b8b1e8",
            "env-vars":["OPENAI_API_KEY"],
            "lock-expiry-seconds": 10
        },
        {
            "name": "stargazers_activity_github_impl",
            "location-oci": "docker.io/getobelisk/demo_stargazers_activity_github_impl:2025-09-28@sha256:8f6fc9b1379b359e085998fa2fd7c966c450327d09770807dfba4b2f75731d72",
            "env-vars": ["GITHUB_TOKEN"],
            "lock-expiry-seconds": 5
        },
        {
            "name": "stargazers_activity_db_turso",
            "location-oci": "docker.io/getobelisk/demo_stargazers_activity_db_turso:2025-09-28@sha256:26b08b3d0c6e430944d8187a00bd9817a83ab89e11ba72d15e7533a758addf33",
            "env-vars": ["TURSO_TOKEN", "TURSO_LOCATION"],
            "lock-expiry-seconds": 5
        }
    ],
    "workflow-list":[
        {
            "name": "stargazers_workflow",
            "location-oci": "docker.io/getobelisk/demo_stargazers_workflow:2025-09-28@sha256:678d85e3e2f89d22794fd1ffc0217bf23510e1349ee150a54d5c82cc2ef75834"
        }
    ],
    "webhook-endpoint-list":[
        {
            "name": "stargazers_webhook",
            "location-oci": "docker.io/getobelisk/demo_stargazers_webhook:2025-09-28@sha256:aa4dfa18d1ad7c1623163eeabb41a415ebad5296fca8f3b957987afcdb2a0f40",
            "routes": [{ "methods": ["POST", "GET"], "path": "" }],
            "env-vars": ["GITHUB_WEBHOOK_SECRET"]
        }
    ]
},
60,
$SKIP_CLEANUP
]
EOF
