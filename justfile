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

# Render the rule catalogue to `gh-pages/index.html`.
# `git_ref` defaults to the current branch name, falling back to the
# commit SHA when checked out detached (e.g., the `actions/checkout`
# default in CI). gen-docs requires a non-empty value.
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
