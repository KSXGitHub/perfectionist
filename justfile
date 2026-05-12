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

# Install cargo-dylint and dylint-link at the version this workspace requires
install-dev-tools:
  #!/usr/bin/env bash
  # Implemented in shell rather than a `_utils` binary so a fresh
  # checkout — where `dylint-link` (the configured linker) is not yet
  # on PATH and `cargo build` therefore can't compile anything — can
  # still run it.
  set -euo pipefail
  version=$(
    awk '
      /^\[\[package\]\]/ { in_pkg = 1; name = ""; version = ""; next }
      in_pkg && /^name *= / { name = $0 }
      in_pkg && /^version *= / { version = $0 }
      in_pkg && /^$/ {
        if (name ~ /"dylint_linting"/) {
          sub(/^version *= *"/, "", version)
          sub(/"$/, "", version)
          print version
          exit
        }
        in_pkg = 0
      }
    ' Cargo.lock
  )
  if [ -z "$version" ]; then
    echo "Could not find dylint_linting version in Cargo.lock" >&2
    exit 1
  fi
  echo "Installing cargo-dylint and dylint-link at version $version"
  cargo install --locked --force --version "$version" cargo-dylint
  cargo install --locked --force --version "$version" dylint-link

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
