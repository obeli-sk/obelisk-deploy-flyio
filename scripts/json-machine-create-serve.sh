#!/usr/bin/env bash

cat <<EOF
{
    "image": "getobelisk/obelisk:0.25.3-ubuntu",
    "init": {
        "swap-size-mb": 256,
        "cmd": ["server", "run", "--config", "/volume/obelisk.toml"]
    },
    "guest": {
        "cpu-kind": "shared",
        "cpus": 1,
        "memory-mb": 256
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
