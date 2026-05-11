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
  cargo build --workspace --all-targets

# Check documentation
doc:
  just gen-docs
  RUSTFLAGS='-D warnings' cargo doc --no-deps --document-private-items

# Run all the lints
lint:
  cargo clippy --workspace --all-targets -- -D warnings

# Run all the tests
test:
  just warmup-integration-tests
  cargo test --workspace --all-targets

# Run perfectionist's own lints on its source
self-lint:
  DYLINT_RUSTFLAGS='-D warnings' cargo dylint --all -- --all-targets

# Pre-warm `target/integration-fixtures`
warmup-integration-tests:
  time cargo run --package _utils --bin warmup -- "$(pwd)"

# Render the rule catalogue to `gh-pages/index.html`
gen-docs out_dir="gh-pages" git_ref="master":
  cargo run --package _gen_docs --bin gen-docs -- "$(pwd)" "{{out_dir}}" --git-ref="{{git_ref}}"
