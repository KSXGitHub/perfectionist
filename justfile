_default:
  @just --list

# Check everything
all:
  just build
  just doc
  just lint
  just test

# Build in debug mode
build:
  cargo build

# Check documentation
doc:
  RUSTFLAGS='-D warnings' run doc

# Run all the lints
lint:
  cargo clippy --all-targets -- -D warnings

# Run all the tests
test:
  cargo test
