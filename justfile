# `.dev-tools/bin` holds workspace-local copies of cargo-dylint and
# dylint-link, installed by the `install-dev-tools` recipe and pinned
# to the `dylint_linting` version in Cargo.lock. Prepending it to
# PATH means every recipe's `cargo dylint` / `dylint-link` (the
# latter via `.cargo/config.toml`) resolves there first, ahead of
# whatever stale copies the developer left under `~/.cargo/bin`.
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
# The `--config linker="cc"` override lets the installer compile on a
# fresh checkout where `dylint-link` (the workspace's linker per
# `.cargo/config.toml`) is not yet on PATH.
install-dev-tools:
  cargo --config 'target."cfg(all())".linker="cc"' run --package _install_dev_tools -- "$(pwd)"

# Print the dylint_linting version pinned in Cargo.lock
dylint-version:
  @cargo --config 'target."cfg(all())".linker="cc"' run --quiet --package _install_dev_tools -- "$(pwd)" --print-version

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
