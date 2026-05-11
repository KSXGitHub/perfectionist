_default:
  @just --list

# Check everything
all:
  just fmt
  just build
  just doc
  just lint
  just test
  just self-lint

# Check format
fmt:
  cargo fmt -- --check

# Build in debug mode
build:
  cargo build

# Check documentation
doc:
  RUSTFLAGS='-D warnings' cargo doc --no-deps --document-private-items

# Run all the lints
lint:
  cargo clippy --all-targets -- -D warnings

# Run all the tests
test:
  cargo test

# Pre-warm `target/integration-fixtures` so the per-test
# `cargo dylint` invocations don't each pay the cost of compiling std
# and the perfectionist plugin from cold.
warmup-integration-tests:
  cargo run --package _utils --bin warmup -- "$(pwd)"

# Run perfectionist's own lints on its source
self-lint:
  DYLINT_RUSTFLAGS='-D warnings' cargo dylint --all -- --all-targets
