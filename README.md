# Obelisk deployment app for fly.io

An [Obelisk](https://obeli.sk) [workflow](workflow/deployer-workflow/impl-flyio/src/lib.rs)
that deploys an Obelisk app on fly.io.

## Setting up
Set up environment variables based on [.envrc-example](.envrc-example).

Set up dependencies based on [dev-deps.txt](dev-deps.txt).

If `direnv` is available:
```sh
cp .envrc-example .envrc
$EDITOR .envrc # change FLY_APP_NAME and FLY_API_TOKEN
direnv allow
```

## Starting the server

Start Obelisk server
```sh
just build serve
# or just `obelisk server run -c obelisk-oci.toml` without building the WASM components locally.
```

## Starting the workflow

### Deploying the deployment app itself

Run the [`app-init`](workflow/deployer-workflow/wit/obelisk-flyio_workflow@1.0.0-beta/workflow.wit) function:
```sh
just app-init "$(./scripts/json-app-init-itself.sh)"
```

While the workflow is running, push the secrets of your `.envrc` to the fly.io app -
either using `fly` command, fly.io's dashboard or using following [script](scripts/secrets-send.sh):

```sh
./scripts/secrets-send.sh .envrc
```

The following secret is required by the app:
* `FLY_API_TOKEN`

When all required secrets are present, the workflow will continue with creating the final VM and health checks.


Sample output:
```
E_01K6HYJ4135FC5CXW14NTGXH65
Locked
BlockedByJoinSet o:1-prepare
BlockedByJoinSet o:2-wait-for-secrets
BlockedByJoinSet o:3-start-final-vm
BlockedByJoinSet o:4-wait-for-health-check
Finished
Execution finished: OK: (no return value)

Execution took 58.933092209s.
```

The execution log can be inspected using the WebUI available at http://localhost:8080 .

<div>
  <img src="doc/trace.png" width="700px"/>
  <div style="width:700px;"><em>Trace view</em></div>
</div>

<div>
  <img src="doc/debug.png" width="700px"/>
  <div style="width:700px;"><em>Debug view</em></div>
</div>


After testing delete the app and its resources:
```sh
fly apps delete $FLY_APP_NAME
```

#### Inception - deploying the app one more time using the deployer on fly.io

Pick a name for the inner app, we will use `inception`.

Kill the local Obelisk server.

Proxy the gRPC port 5005 of the `obelisk` VM:
```sh
flyctl proxy 5005 $(fly machine list | grep obelisk | awk '{print $6}')
```

Verify the port tunneling works:
```sh
obelisk client component list
```

Now run `app-init` with a new fly app name:
```sh
just app-init "$(FLY_APP_NAME=inception ./scripts/json-app-init-itself.sh)"
```

Push the secret to the outer webhook:
```sh
URL="https://${FLY_APP_NAME}.fly.dev" FLY_APP_NAME=inception ./scripts/secrets-send.sh .envrc
```

To show the web console, proxy the port 8080 as well.

Don't forget to delete the inner app afterwards:
```sh
fly apps delete inception
```

### Stargazers
Similar to the process above, but deploying the [Stargazers Demo app](https://github.com/obeli-sk/demo-stargazers) requires setting up, see the project's readme for details.

Run the [`app-init`](workflow/deployer-workflow/wit/obelisk-flyio_workflow@1.0.0-beta/workflow.wit) function:
```sh
just app-init "$(./scripts/json-app-init-stargazers.sh)"
```

Push the secrets using stargazers' `.envrc` file.
```sh
./scripts/secrets-send.sh ../stargazers/.envrc
```

The follwing secrets are required by the app:
* OPENAI_API_KEY
* GITHUB_TOKEN
* TURSO_TOKEN
* TURSO_LOCATION
* GITHUB_WEBHOOK_SECRET

# Using Fly.io activities directly

Check out the [components-flyio](https://github.com/obeli-sk/components-flyio) repo on how to interact with Fly.io, including:
* Apps
* Volumes
* VMs
* Secrets
