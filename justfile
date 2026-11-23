# Run the local server.
dev:
    cargo run --locked -p concentus_server

# Format the workspace with a single import group.
fmt:
    cargo fmt --all -- --config group_imports=One

# Check workspace formatting and compilation.
check:
    cargo fmt --all -- --check --config group_imports=One
    cargo check --locked --workspace

# Build the native server.
build:
    cargo build --locked -p concentus_server
