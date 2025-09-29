Start Obelisk
```sh
just build serve
```

Initialize fly.io app - creates an app, volume, temp VM, verifies the stargazer app and waits for secrets.
```sh
obelisk client execution submit -f obelisk-flyio:workflow/workflow@1.0.0-beta.app-init \
"$(./scripts/json-app-init-stargazers.sh)"
```

In another terminal push secrets from stargazers' `.envrc` file into the fly.io app.
```sh
./scripts/secrets-send.sh .envrc
```

When all required secrets are present, the `app-init` workflow finishes.


## Utils

Launch a VM:
```sh
export VOLUME_ID=..
MACHINE_ID=$(obelisk client execution submit -f --json obelisk-flyio:activity-fly-http/machines@1.0.0-beta.create -- \
\"$FLY_APP_NAME\" \"$FLY_MACHINE_NAME\" "$(./scripts/json-machine-create.sh)" \"$FLY_REGION\" \
| jq -r '.[-1].ok')
```

Exec verify:
```sh
obelisk client execution submit -f \
obelisk-flyio:activity-fly-http/machines@1.0.0-beta.exec \
-- \
\"$FLY_APP_NAME\" \
\"$MACHINE_ID\" \
'["obelisk", "server", "verify", "-c", "/volume/obelisk.toml"]'
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
