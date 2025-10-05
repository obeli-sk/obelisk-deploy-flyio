gen-obelisk-ext:
	scripts/obelisk-generate-extensions.sh

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

app-init:
	obelisk client execution submit -f .../workflow.app-init "$(./scripts/json-app-init-stargazers.sh)"

app-init-no-cleanup:
	SKIP_CLEANUP=true obelisk client execution submit -f .../workflow.app-init "$(./scripts/json-app-init-stargazers.sh)"

secrets:
	./scripts/secrets-send.sh ../stargazers/.envrc

secrets-y:
	SEND_ALL=true ./scripts/secrets-send.sh ../stargazers/.envrc
