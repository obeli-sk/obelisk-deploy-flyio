#!/usr/bin/env bash

cat <<EOF
[
"stargazers2",
{
    "size-in-gb":1,
    "name": "stargazers"
},
{
    "activity-wasm-list":[
        {
            "name": "stargazers_activity_llm_chatgpt",
            "location-oci": "docker.io/getobelisk/demo_stargazers_activity_llm_openai:2025-09-01@sha256:9cde0e4feb0ad69f476eeeb274628ea230314f557e1aa1a3bfa3ee05ecc584ae",
            "env-vars":["OPENAI_API_KEY"],
            "lock-expiry-seconds": 10
        },
        {
            "name": "stargazers_activity_github_impl",
            "location-oci": "docker.io/getobelisk/demo_stargazers_activity_github_impl:2025-09-01@sha256:1dd39ff12539353c12ffa36f225fcd003a7cf57ebed49c3db78f259cf556fd6c",
            "env-vars": ["GITHUB_TOKEN"],
            "lock-expiry-seconds": 5
        },
        {
            "name": "stargazers_activity_db_turso",
            "location-oci": "docker.io/getobelisk/demo_stargazers_activity_db_turso:2025-09-01@sha256:87ec6d390e25640e1d968fb419086742c53f5731b29f86d567212cb46b4fa2ed",
            "env-vars": ["TURSO_TOKEN", "TURSO_LOCATION"],
            "lock-expiry-seconds": 5
        },
        {
            "name": "mlist_activity_db_turso",
            "location-oci": "docker.io/getobelisk/mlist_activity_db_turso:2025-05-08-2@sha256:55aa06ebd6f8736be14343a843cdc6488b88b412ce2e878c47b81f9b18a9c326",
            "env-vars": ["MLIST_TURSO_TOKEN", "MLIST_TURSO_LOCATION"],
            "lock-expiry-seconds": 5
        }
    ],
    "workflow-list":[
        {
            "name": "stargazers_workflow",
            "location-oci": "docker.io/getobelisk/demo_stargazers_workflow:2025-09-01@sha256:11c662196dd579f3f19e61caf565017a61f944140fa8c92993c742e69a3d8da9"
        }
    ],
    "webhook-endpoint-list":[
        {
            "name": "stargazers_webhook",
            "location-oci": "docker.io/getobelisk/demo_stargazers_webhook:2025-09-01@sha256:0fb0042da931bedea3f616a9eca40468032625d0fd69333dc2733f576a8887d9",
            "routes": [{ "methods": ["POST", "GET"], "path": "" }],
            "env-vars": ["GITHUB_WEBHOOK_SECRET"]
        },
        {
            "name": "mlist_webhook",
            "location-oci": "docker.io/getobelisk/mlist_webhook:2025-05-08-2@sha256:34e0a7af37fc2cf5b6863689c3c72273b6e05d2e3c4548663a4b160850a6d57c",
            "routes": [{ "methods": ["POST"], "path": "/mlist" }]
        }
    ]
}
]
EOF
