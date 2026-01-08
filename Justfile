clean:
	cargo clean

build:
	(cd activity/obelisk-client && cargo build --release)
	(cd workflow/deployer-workflow/impl-flyio && cargo build --profile release_workflow)
	(cd webhook/healthcheck && cargo build --profile release_webhook)

test:
	cargo nextest run --workspace

test-integration:
	(cd activity/obelisk-client && TEST_ENDPOINT_URL=http://localhost:5005 cargo nextest run -- --ignored)

verify:
	obelisk server verify --config ${CONFIG:-obelisk-local.toml}

serve:
	obelisk server run --config ${CONFIG:-obelisk-local.toml}

serve-oci:
	obelisk server run --config ${CONFIG:-obelisk-oci.toml}

app-init params:
	obelisk client execution submit -f .../workflow.app-init '{{params}}'

start-restart-should-persist-state params:
	obelisk client execution submit -f .../testing.start-restart-should-persist-state '{{params}}'
