#!/usr/bin/env bash
set -euo pipefail

SEND_ALL=${SEND_ALL:-false}

FILE="${1:-.envrc}"
shift || true

# If extra arguments are given, treat them as the set of keys to send non-interactively.
declare -A SELECTED_KEYS=()
for k in "$@"; do
  SELECTED_KEYS["$k"]=1
done
FILTER_KEYS=$(( ${#SELECTED_KEYS[@]} > 0 ? 1 : 0 ))

if [ "$FILTER_KEYS" -eq 0 ]; then
  exec 3<&0 # save stdin as FD 3 for interactive prompts
fi

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

    if [ "$FILTER_KEYS" -eq 1 ]; then
      # Non-interactive: only send keys listed on the command line
      if [ -n "${SELECTED_KEYS[$key]+x}" ]; then
        echo "Sending: $key"
        fly secrets set --stage --app "$FLY_APP_NAME" "$key=$val"
        unset "SELECTED_KEYS[$key]"
      fi
    else
      echo "Found: $key"
      if [ "$SEND_ALL" = "true" ] || { read -u 3 -p "Send secret to app '$FLY_APP_NAME'? (y/n) " confirm && [[ "$confirm" == "y" ]]; }; then
        fly secrets set --stage --app "$FLY_APP_NAME" "$key=$val"
      fi
    fi
  fi
done < "$FILE"

# Warn about keys that were requested but not found in the file
if [ "$FILTER_KEYS" -eq 1 ]; then
  for missing in "${!SELECTED_KEYS[@]}"; do
    echo "Warning: key '$missing' not found in $FILE" >&2
  done
fi
