export PATH := justfile_directory() + "/.dev-tools/bin:" + env_var("PATH")

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

test_tmp_dir := env_var_or_default("TMPDIR", "/tmp") + "/perfectionist-tests"

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
  RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --document-private-items {{locked}}

# Run all the lints
lint:
  cargo clippy --workspace --all-targets {{locked}} -- -D warnings

# Run all the tests
test:
  mkdir -pv "{{test_tmp_dir}}"
  just warmup-integration-tests
  TMPDIR="{{test_tmp_dir}}" cargo test --workspace --all-targets {{locked}}

# Run perfectionist's own lints on its source
self-lint:
  DYLINT_RUSTFLAGS='-D warnings' cargo dylint --all -- --workspace --all-targets {{locked}}

# Pre-warm `target/integration-fixtures`
warmup-integration-tests:
  cargo run {{locked}} --package _utils --bin warmup -- "$(pwd)"

# Install cargo-dylint and dylint-link into `.dev-tools/`
install-dev-tools:
  cargo --config 'target."cfg(all())".linker="cc"' run --locked --target-dir target/dev-tools-cc --package _dev_tools -- "$(pwd)" install

# Set up git hooks
install-git-hooks:
  git config core.hooksPath .githooks

# Uninstall git hooks
uninstall-git-hooks:
  git config --unset core.hooksPath 2>/dev/null || true

# Print the dylint_linting version pinned in Cargo.lock
dylint-version:
  @cargo --config 'target."cfg(all())".linker="cc"' run --locked --target-dir target/dev-tools-cc --quiet --package _dev_tools -- "$(pwd)" dylint-version

# Append `version=<dylint version>` to $GITHUB_OUTPUT (for CI)
gha-dylint-version:
  cargo --config 'target."cfg(all())".linker="cc"' run --locked --target-dir target/dev-tools-cc --package _dev_tools -- "$(pwd)" gha-dylint-version

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

# Check the docs-site JavaScript: formatting, types, then unit tests
check-js:
  just fmt-js
  just check-js-types
  just test-js

# Print the Biome version pinned in the Check JS workflow
biome-version:
  @sed -n "s/.*BIOME_VERSION: '\([^']*\)'.*/\1/p" "{{justfile_directory()}}/.github/workflows/check-js.yaml"

# Install the pinned Biome into `.dev-tools/bin`
install-biome:
  #!/usr/bin/env bash
  set -euo pipefail
  root_dir="{{justfile_directory()}}"
  version="$(just biome-version)"
  if [ -z "$version" ]; then
    echo "could not read BIOME_VERSION from .github/workflows/check-js.yaml" >&2
    exit 1
  fi
  mkdir -p "$root_dir/.dev-tools/bin"
  curl --proto '=https' --tlsv1.2 -sSfL \
    "https://github.com/biomejs/biome/releases/download/@biomejs/biome@$version/biome-linux-x64" \
    -o "$root_dir/.dev-tools/bin/biome"
  chmod +x "$root_dir/.dev-tools/bin/biome"

# Check the docs-site JavaScript's formatting
fmt-js:
  #!/usr/bin/env bash
  set -euo pipefail
  cd "{{justfile_directory()}}"
  # The file list comes from git rather than from a directory walk, so the
  # set this checks is exactly the set CI checks.
  git ls-files '*.js' '*.mjs' | tr '\n' '\0' | xargs -0 biome format

# Format the docs-site JavaScript in place
write-fmt-js:
  #!/usr/bin/env bash
  set -euo pipefail
  cd "{{justfile_directory()}}"
  git ls-files '*.js' '*.mjs' | tr '\n' '\0' | xargs -0 biome format --write

# Type-check the docs-site JavaScript from its JSDoc annotations
check-js-types:
  #!/usr/bin/env bash
  set -euo pipefail
  root_dir="{{justfile_directory()}}"
  pnpm --dir "$root_dir" install --frozen-lockfile
  pnpm --dir "$root_dir" exec tsc --noEmit --project "$root_dir/tsconfig.json"

# Run the docs-site JavaScript unit tests
test-js:
  node "{{justfile_directory()}}/tools/gen-docs/tests/run.mjs"

# Minify a gen-docs output directory's CSS, JS, and SVG assets in place.
minify-docs site_dir="gh-pages":
  #!/usr/bin/env bash
  set -euo pipefail
  root_dir="{{justfile_directory()}}"
  pnpm --dir "$root_dir" install --frozen-lockfile
  cd "{{site_dir}}"
  shopt -s nullglob
  # CSS: lightningcss minifies every file in place.
  css=(*.css)
  if [ "${#css[@]}" -gt 0 ]; then
    pnpm exec lightningcss --minify --targets 'ie 11' --sourcemap --output-dir . "${css[@]}"
  fi
  # JS: terser minifies every file in place.
  for js in *.js; do
    pnpm exec terser "$js" --compress --mangle --source-map "url='$js.map',includeSources=true" --output "$js"
  done
  # SVG: svgo minifies every file in place.
  svg=(*.svg)
  if [ "${#svg[@]}" -gt 0 ]; then
    pnpm exec svgo --quiet --folder .
  fi
