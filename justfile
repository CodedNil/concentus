# Run the local server.
dev:
    topcoat dev -p concentus_server

# Format Rust and Topcoat views.
fmt:
    cargo fmt --all
    topcoat fmt crates

# Format the workspace, then check compilation.
check: fmt
    cargo check --locked --workspace

# Build the native server.
build:
    cargo build --locked -p concentus_server
