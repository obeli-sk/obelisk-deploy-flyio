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
	obelisk server verify --config obelisk-local.toml

serve:
	obelisk server run --config obelisk-local.toml
