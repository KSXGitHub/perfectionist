export PATH := justfile_directory() + "/.dev-tools/bin:" + env_var("PATH")

# Set PERFECTIONIST_CARGO_LOCKED=true to pass `--locked` to the cargo invocations the
# gating CI recipes (`build`, `doc`, `lint`, `test`, `self-lint`) reach,
# including the warmup / doc-generation recipes they delegate to. Empty,
# `false`, and unset all leave the lockfile writable; any other value
# is rejected.
perfectionist_cargo_locked := env_var_or_default("PERFECTIONIST_CARGO_LOCKED", "")
locked := if perfectionist_cargo_locked == "true" {
    "--locked"
  } else if perfectionist_cargo_locked == "false" {
    ""
  } else if perfectionist_cargo_locked == "" {
    ""
  } else {
    error("PERFECTIONIST_CARGO_LOCKED must be 'true', 'false', empty, or unset; got: " + perfectionist_cargo_locked)
  }

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
  cargo build --workspace --all-targets {{locked}}

# Check documentation
doc:
  just gen-docs
  just check-rules-md
  RUSTFLAGS='-D warnings' cargo doc --no-deps --document-private-items {{locked}}

# Run all the lints
lint:
  cargo clippy --workspace --all-targets {{locked}} -- -D warnings

# Run all the tests
test:
  just warmup-integration-tests
  cargo test --workspace --all-targets {{locked}}

# Run perfectionist's own lints on its source
self-lint:
  DYLINT_RUSTFLAGS='-D warnings' cargo dylint --all -- --all-targets {{locked}}

# Pre-warm `target/integration-fixtures`
warmup-integration-tests:
  time cargo run {{locked}} --package _utils --bin warmup -- "$(pwd)"

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
  cargo run {{locked}} --package _gen_docs --bin gen-docs -- --root "$(pwd)" html "{{out_dir}}" --git-ref="$ref"

# Regenerate the in-tree markdown catalogue under `rules/`.
gen-rules-md rules_dir="rules":
  cargo run {{locked}} --package _gen_docs --bin gen-docs -- --root "$(pwd)" write-md "{{rules_dir}}"

# Verify the in-tree markdown catalogue is in sync with `src/rules/`.
check-rules-md rules_dir="rules":
  cargo run {{locked}} --package _gen_docs --bin gen-docs -- --root "$(pwd)" check-md "{{rules_dir}}"
