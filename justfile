export PATH := justfile_directory() + "/.dev-tools/bin:" + env_var("PATH")

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

# Install cargo-dylint and dylint-link into `.dev-tools/`
install-dev-tools:
  cargo --config 'target."cfg(all())".linker="cc"' run --locked --package _dev_tools -- "$(pwd)" install

# Print the dylint_linting version pinned in Cargo.lock
dylint-version:
  @cargo --config 'target."cfg(all())".linker="cc"' run --locked --quiet --package _dev_tools -- "$(pwd)" dylint-version

# Append `version=<dylint version>` to $GITHUB_OUTPUT (for CI)
gha-dylint-version:
  cargo --config 'target."cfg(all())".linker="cc"' run --locked --package _dev_tools -- "$(pwd)" gha-dylint-version

# Render the rule catalogue to `gh-pages/index.html`.
gen-docs out_dir="gh-pages" git_ref="":
  #!/usr/bin/env bash
  set -euo pipefail
  ref="{{git_ref}}"
  if [ -z "$ref" ]; then
    ref="$(git branch --show-current)"
  fi
  if [ -z "$ref" ]; then
    ref="$(git rev-parse HEAD)"
  fi
  cargo run --package _gen_docs --bin gen-docs -- "$(pwd)" "{{out_dir}}" --git-ref="$ref"
