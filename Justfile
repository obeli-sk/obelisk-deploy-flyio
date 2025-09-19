gen-obelisk-ext:
	scripts/obelisk-generate-extensions.sh

gen-wit-bindgen:
	(cd workflow/deployer-workflow/impl-flyio && wit-bindgen rust --generate-all --out-dir ./src/generated wit)


build: gen-wit-bindgen
	(cd workflow/deployer-workflow/impl-flyio && cargo build --release)

serve:
	obelisk server run --config obelisk-local.toml

test:
	cargo nextest run
