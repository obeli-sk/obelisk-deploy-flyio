#!/usr/bin/env bash

cat <<EOF
{
    "image": "getobelisk/obelisk:0.24.2-ubuntu",
    "init": {
        "swap-size-mb": 256
    },
    "guest": {
        "cpu-kind": "shared",
        "cpus": 1,
        "memory-mb": 256
    },
    "init": {
        "entrypoint":["/usr/bin/sleep"],
        "cmd": ["infinity"]
    },
    "restart": {
        "policy": "no"
    }$(if [ -n "$VOLUME_ID" ]; then
        echo ',
    "mounts": [
        {
            "volume": "'"$VOLUME_ID"'",
            "path": "/volume"
        }
    ]'
    fi)
}
EOF
