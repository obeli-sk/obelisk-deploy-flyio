clean:
	cargo clean

build:
	(cd workflow/deployer-workflow/impl-flyio && cargo build --profile release_workflow)
	(cd webhook/healthcheck && cargo build --profile release_webhook)

test:
	cargo nextest run

verify:
	obelisk server verify --config ${CONFIG:-obelisk-local.toml}

serve:
	obelisk server run --config ${CONFIG:-obelisk-local.toml}

serve-oci:
	obelisk server run --config ${CONFIG:-obelisk-oci.toml}

app-init params:
	obelisk client execution submit -f obelisk-flyio:workflow/workflow@1.0.0-beta.app-init '{{params}}'
