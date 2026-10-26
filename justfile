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

# Re-record every `// run-rustfix` fixture's fixed form
bless-fixed:
  #!/usr/bin/env bash
  set -euo pipefail
  mkdir -pv "{{test_tmp_dir}}"
  # A staged file left by an earlier run would be read as this run's,
  # so what each fixture produced is what is left after clearing them.
  find "{{test_tmp_dir}}" -name '*.stage-id.fixed' -delete
  # The run is expected to fail wherever a `.diff` is out of date, and
  # compiletest writes what it produced under the test's own directory
  # either way, which is what gets recorded. `--no-fail-fast` because a
  # fixture of one test binary is recorded from a run another binary
  # also failed in.
  TMPDIR="{{test_tmp_dir}}" cargo test --workspace --all-targets --no-fail-fast {{locked}} || true
  # `--others` so a `.diff` created for a new fixture and not yet
  # added is recorded too.
  for committed in $(git ls-files --cached --others --exclude-standard '*.diff'); do
    # compiletest names its output after the fixture's path below the
    # `src_base` it was handed, which is the copy of `ui` or of
    # `ui-toml`, so the committed path maps onto the staged one by
    # dropping that first component.
    staged="{{test_tmp_dir}}/${committed#*/}"
    staged="${staged%.diff}.stage-id.fixed"
    # compiletest stages what it produced only where that differs from
    # the fixed form the `.diff` gave it, so nothing staged means the
    # `.diff` already describes the rewrite.
    if [ ! -f "$staged" ]; then
      echo "already recorded $committed"
      continue
    fi
    fixture="${committed%.diff}.rs"
    fixed=$(mktemp)
    # The copy a fixture is run from carries the injected
    # `normalize-stderr-test` directives, which the fixture does not.
    sed '/^\/\/ normalize-stderr-test: /d' "$staged" > "$fixed"
    # `git diff` names the two paths it was handed, where the fixture's
    # own name on both sides reads as what the diff turns into its
    # fixed form. It also exits non-zero when they differ, which is the
    # expected case here.
    { git diff --no-index --unified=3 "$fixture" "$fixed" || true; } \
      | sed -n '/^--- /,$p' \
      | sed "1s|.*|--- a/$(basename "$fixture")|; 2s|.*|+++ b/$(basename "$fixture")|" \
      > "$committed"
    rm -f "$fixed"
    echo "re-recorded $committed"
  done
  TMPDIR="{{test_tmp_dir}}" cargo test --workspace --all-targets --no-fail-fast {{locked}}

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

# Check the docs-site JavaScript: formatting, types, lints, then unit tests
check-js:
  just fmt-js
  just check-js-types
  just lint-js
  just test-js

# Check the docs-site JavaScript's formatting
fmt-js:
  #!/usr/bin/env bash
  set -euo pipefail
  root_dir="{{justfile_directory()}}"
  pnpm --dir "$root_dir" install --frozen-lockfile
  pnpm --dir "$root_dir" exec oxfmt --check '**/*.js' '**/*.mjs'

# Format the docs-site JavaScript in place
write-fmt-js:
  #!/usr/bin/env bash
  set -euo pipefail
  root_dir="{{justfile_directory()}}"
  pnpm --dir "$root_dir" install --frozen-lockfile
  pnpm --dir "$root_dir" exec oxfmt --write '**/*.js' '**/*.mjs'

# Type-check the docs-site JavaScript from its JSDoc annotations
check-js-types:
  #!/usr/bin/env bash
  set -euo pipefail
  root_dir="{{justfile_directory()}}"
  pnpm --dir "$root_dir" install --frozen-lockfile
  pnpm --dir "$root_dir" exec tsc --noEmit --project "$root_dir/tsconfig.json"

# Lint the docs-site JavaScript, types included
lint-js:
  #!/usr/bin/env bash
  set -euo pipefail
  root_dir="{{justfile_directory()}}"
  pnpm --dir "$root_dir" install --frozen-lockfile
  pnpm --dir "$root_dir" exec oxlint --type-aware

# Apply what oxlint can fix to the docs-site JavaScript, then reformat
fix-js:
  #!/usr/bin/env bash
  set -euo pipefail
  root_dir="{{justfile_directory()}}"
  pnpm --dir "$root_dir" install --frozen-lockfile
  pnpm --dir "$root_dir" exec oxlint --type-aware --fix
  just write-fmt-js

# Run the docs-site JavaScript unit tests
test-js:
  node "{{justfile_directory()}}/tools/gen-docs/tests/run.mjs"

# Minify a gen-docs output directory's CSS and JS in place.
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
