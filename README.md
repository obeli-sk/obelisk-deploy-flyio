
Start the server
```sh
just build serve
```

Initialize app
```sh
obelisk client execution submit -f obelisk-flyio:workflow/workflow@1.0.0-beta.app-init \
"$(./scripts/json-app-init-stargazers.sh)"
```

Launch a VM:
```sh
export VOLUME_ID=..
MACHINE_ID=$(obelisk client execution submit -f --json obelisk-flyio:activity-fly-http/machines@1.0.0-beta.create -- \
\"$FLY_APP_NAME\" \"$FLY_MACHINE_NAME\" "$(./scripts/json-machine-create.sh)" \"$FLY_REGION\" \
| jq -r '.[-1].ok.ok')
```

Get the VM:
```sh
obelisk client execution submit -f obelisk-flyio:activity-fly-http/machines@1.0.0-beta.get -- \
\"$FLY_APP_NAME\" \"$MACHINE_ID\"
```

Delete the VM:
```sh
obelisk client execution submit -f obelisk-flyio:activity-fly-http/machines@1.0.0-beta.delete -- \
\"$FLY_APP_NAME\" \"$MACHINE_ID\" true
```


Delete the app:
```sh
obelisk client execution submit -f obelisk-flyio:activity-fly-http/apps@1.0.0-beta.delete -- \
\"$FLY_APP_NAME\" true
```
