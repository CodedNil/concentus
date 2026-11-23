# Run the local server.
dev:
    cargo run --locked -p concentus_server

# Check workspace formatting and compilation.
check:
    cargo fmt --all --check
    cargo check --locked --workspace

# Build the native server.
build:
    cargo build --locked -p concentus_server
