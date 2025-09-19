gen-obelisk-ext:
	scripts/obelisk-generate-extensions.sh

build:
	(cd workflow/deployer-workflow/impl-flyio && cargo build --release)

serve:
	obelisk server run --config obelisk-local.toml

test:
	cargo nextest run
