clean:
	cargo clean

build:
	(cd workflow/deployer-workflow/ && cargo build --profile release_workflow)
	(cd webhook/healthcheck && cargo build --profile release_webhook)

test:
	cargo nextest run --workspace

test-integration:
	(cd activity/obelisk-&& TEST_ENDPOINT_URL=http://localhost:5005 cargo nextest run -- --ignored)

verify *params:
	obelisk server verify --server-config server.toml --deployment obelisk-local.toml {{params}}
	obelisk server verify --server-config server-postgres.toml --deployment obelisk-local-postgres.toml {{params}}
	obelisk server verify --server-config server.toml --deployment obelisk-oci.toml {{params}}

serve:
	obelisk server run --server-config server.toml --deployment ${CONFIG:-obelisk-local.toml}

serve-oci:
	obelisk server run --server-config server.toml --deployment ${CONFIG:-obelisk-oci.toml}

app-init params:
	obelisk execution submit -f .../workflow.app-init '{{params}}'

start-restart-should-persist-state *params:
	obelisk execution submit -f .../testing.start-restart-should-persist-state {{params}}
