test:
	cargo fmt -- --check
	cargo test
	cargo build --features deny_warnings
	cargo run --example cli --features deny_warnings -- target/debug/feeless
	cargo check --no-default-features --features deny_warnings
	cargo check --no-default-features --features deny_warnings --features pcap
	cargo check --no-default-features --features deny_warnings --features node
	cargo check --no-default-features --features deny_warnings --features rpc_client
	cargo check --no-default-features --features deny_warnings --features rpc_server

cli_example:
	cargo build
	cargo run --example cli -- target/debug/feeless

fix:
	cargo fix
	cargo fmt

# Build a docker image, similar to the published one.
docker:
	docker build . --progress=plain -t feeless

# You'll need: `cargo install cargo-release`
release:
	cargo release patch
