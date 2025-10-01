gen-obelisk-ext:
	scripts/obelisk-generate-extensions.sh

build:
	(cd workflow/deployer-workflow/impl-flyio && cargo build --release)

test:
	cargo nextest run

verify:
	obelisk server verify --config obelisk-local.toml

serve:
	obelisk server run --config obelisk-local.toml
