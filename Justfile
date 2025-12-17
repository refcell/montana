set positional-arguments
alias t := test
alias f := fix
alias b := build
alias be := bench
alias c := clean
alias h := hack
alias u := check-udeps
alias wt := watch-test
alias wc := watch-check
alias s := shadow
alias m := montana

# Default to display help menu
default:
    @just --list

# Runs all ci checks.
ci: fix check lychee zepter

# Performs lychee checks, installing the lychee command if necessary
lychee:
  @command -v lychee >/dev/null 2>&1 || cargo install lychee
  lychee --config ./lychee.toml .

# Checks formatting, clippy, and tests
check: check-format check-clippy test

# Fixes formatting and clippy issues
fix: format-fix clippy-fix zepter-fix

# Runs zepter feature checks, installing zepter if necessary
zepter:
  @command -v zepter >/dev/null 2>&1 || cargo install zepter
  zepter --version
  zepter format features
  zepter

# Fixes zepter feature formatting.
zepter-fix:
  @command -v zepter >/dev/null 2>&1 || cargo install zepter
  zepter format features --fix

# Runs tests across workspace with all features enabled
test:
    @command -v cargo-nextest >/dev/null 2>&1 || cargo install cargo-nextest
    RUSTFLAGS="-D warnings" cargo nextest run --workspace --all-features

# Runs cargo hack against the workspace
hack:
  cargo hack check --feature-powerset --no-dev-deps

# Checks formatting
check-format:
    cargo +nightly fmt --all -- --check

# Fixes any formatting issues
format-fix:
    cargo fix --allow-dirty --allow-staged
    cargo +nightly fmt --all

# Checks clippy
check-clippy:
    cargo clippy --all-targets -- -D warnings

# Fixes any clippy issues
clippy-fix:
    cargo clippy --all-targets --fix --allow-dirty --allow-staged

# Builds the workspace with release
build:
    cargo build --release

# Builds all targets in debug mode
build-all-targets:
    cargo build --all-targets

# Builds the workspace with maxperf
build-maxperf:
    cargo build --profile maxperf

# Builds the montana binary
build-montana:
    cargo build --bin montana

# Cleans the workspace
clean:
    cargo clean

# Checks if there are any unused dependencies
check-udeps:
  @command -v cargo-udeps >/dev/null 2>&1 || cargo install cargo-udeps
  cargo +nightly udeps --workspace --all-features --all-targets

# Watches tests
watch-test:
    cargo watch -x test

# Watches checks
watch-check:
    cargo watch -x "fmt --all -- --check" -x "clippy --all-targets -- -D warnings" -x test

# Runs all benchmarks (excludes unit tests)
bench *ARGS:
    cargo bench -p montana-local --bench pipeline -- {{ARGS}}

# Runs pipeline benchmarks
bench-pipeline *ARGS:
    cargo bench -p montana-local --bench pipeline -- {{ARGS}}

# Runs the analyze binary in roundtrip mode
roundtrip *ARGS:
    cargo run --bin analyze -- --mode roundtrip {{ARGS}}

# Runs the shadow TUI for chain shadowing simulation
shadow *ARGS:
    cargo run --release -p shadow -- {{ARGS}}

# Run the montana node in executor mode against Base mainnet
run-montana-executor:
    cargo run --bin montana -- --rpc-url https://base-mainnet-reth-rpc-donotuse.cbhq.net:8545 --mode executor historical --start 39554271 --end 39554273

# Run the montana node in sequencer mode with in-memory batch submission
run-montana-inmemory:
    cargo run --bin montana -- --rpc-url https://base-mainnet-reth-rpc-donotuse.cbhq.net:8545 --batch-mode in-memory historical --start 39554271 --end 39554273

# Run the montana node in dual mode (default: sequencer + validator) with Anvil batch submission against Base mainnet
run-montana:
    cargo run --bin montana -- --rpc-url https://base-mainnet-reth-rpc-donotuse.cbhq.net:8545 historical --start 39554271 --end 39554273

# Alias for running montana
montana *ARGS:
    cargo run --release -p montana -- {{ARGS}}
