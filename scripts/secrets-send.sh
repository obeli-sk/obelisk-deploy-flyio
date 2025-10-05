#!/usr/bin/env bash
set -euo pipefail

SEND_ALL=${SEND_ALL:-false}

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
    if [ "$SEND_ALL" = "true" ] || { read -u 3 -p "Send to server? (y/n) " confirm && [[ "$confirm" == "y" ]]; }; then
      curl --write-out "%{url_effective} %{http_code}\n" --fail localhost:9090/ \
        -H "Content-Type: application/json" \
        -d '{"app_name":"'"$FLY_APP_NAME"'","name":"'"$key"'","value":"'"$val"'"}'
    fi
  fi
done < "$FILE"
