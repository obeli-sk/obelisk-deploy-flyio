#!/usr/bin/env bash
set -euo pipefail

exec 3<&0 # save stdin as FD 3

FILE="${1:-.envrc}"

while IFS= read -r line; do
  # Match lines like: export foo=bar or export foo="bar"
  if [[ $line =~ ^export[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)=(.*)$ ]]; then
    key="${BASH_REMATCH[1]}"
    val="${BASH_REMATCH[2]}"

    # Strip surrounding quotes if any
    val="${val%\"}"
    val="${val#\"}"
    val="${val%\'}"
    val="${val#\'}"

    echo "Found: $key"
    read -u 3 -p "Send to server? (y/n) " confirm
    if [[ "$confirm" == "y" ]]; then
      curl --fail localhost:9090/ \
        -X POST \
        -H "Content-Type: application/json" \
        -d '{"app_name":"'"$FLY_APP_NAME"'","name":"'"$key"'","value":"'"$val"'"}'
    fi
  fi
done < "$FILE"
