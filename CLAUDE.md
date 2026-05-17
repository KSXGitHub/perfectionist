# Implementing rules from `planned-rules/`

This repository's lint rules are documented in `planned-rules/`
before they are implemented. Each markdown file describes one
rule's intent, configuration knobs, examples, implementation
notes, and difficulty. Cross-cutting conventions (parser style,
lint-name namespacing) live in
`planned-rules/IMPLEMENTATION_CONVENTIONS.md`.

This guide tells you how to implement those rules and how to
keep the catalogue in sync as you go.

## Before you write code

Read three things first, in this order:

1. **The rule's own file**, `planned-rules/<rule-name>.md`. It
   specifies the lint's identifier, configuration, the precise
   trigger predicate ("What to lint"), suggested fixes, and an
   estimated difficulty. Don't second-guess the design without
   first checking the rule file's existing examples and rationale
   — the design has usually been argued over already in the PR
   that produced the planning file.
2. **`planned-rules/README.md`** — the index of all rules, plus
   the "Out of scope" list at the bottom. The index entry is the
   one-sentence summary; check that the rule you're implementing
   still says what you think it says.
3. **`planned-rules/IMPLEMENTATION_CONVENTIONS.md`** — applies
   to every rule. Currently covers two cross-cutting conventions:
   - **Parser style.** Non-trivial string scanners (URLs,
     emails, format templates, markdown spans, serde-attribute
     type literals) are written as parser-combinator-style
     `take_*` functions, not regex.
   - **Lint name namespacing.** Every lint registers under the
     `perfectionist` tool namespace via
     `rustc_session::declare_tool_lint!`. The planning files use
     the unqualified form (`qualified_paths`) for readability;
     the registered name is `perfectionist::qualified_paths`.

If the rule is one of several that share a helper (markdown
exclusion, format-string parsing, URL discovery, unicode-width
measurement), check whether the helper already exists in the
codebase before writing a new one. Sibling-rule references in the
planning files identify shared infrastructure.

## One rule per file, one `Config` per rule

The catalogue is organised so that each rule has exactly one
source file at `src/rules/<rule_name>.rs` and exactly one `Config`
struct keyed by the rule's full namespaced name. The convention
has two consequences for the implementer:

1. **Before writing code, check whether the rule is actually one
   rule.** A planning file that bundles several independently-
   triggered checks under one banner is usually better
   implemented as several rules. If the sub-checks can be cleanly
   separated — distinct trigger predicates, disjoint
   configuration, no shared diagnostic — split the planning file
   into one rule per sub-check before you start. (Historical
   example: an early `single_letter_names` rule bundled four
   independently-configured checks for generics, `let` bindings,
   function parameters, and closure parameters; each lives in its
   own file today. See
   <https://github.com/KSXGitHub/perfectionist/pull/43>)

2. **When writing code, give each rule its own file and `Config`.**
   The file name matches the rule name (snake_case, no
   `perfectionist::` prefix). `CONFIG_KEY` is the full namespaced
   name (`perfectionist::<rule_name>`). The `Config` struct holds
   only the fields the rule actually reads — fields nominally
   "about" the rule but consumed by a different rule belong in
   that other rule's `Config`. If two rules genuinely share a
   helper function or type, factor it into `src/common.rs` (for
   trivial cross-rule utilities) or a dedicated
   crate-internal module rather than co-housing the rules in one
   file.

3. **When a rule grows past one screenful, split it into a
   directory module beside the flat `.rs` entry.** The crate's own
   `perfectionist::flat_module_pattern` lint forbids the `mod.rs`
   form, so the layout is `src/rules/<rule>.rs` next to
   `src/rules/<rule>/<concern>.rs`. The flat `.rs` entry keeps the
   `declare_tool_lint!` block, the `register_lint` / `register_pass`
   functions, the `EarlyLintPass` / `LateLintPass` driver, and any
   process-wide state (`static PENDING_VIOLATIONS`, etc.). Common
   submodule names that have emerged:
   - `config` — `Config` struct, default lists, in-memory rule state.
   - `early` / `late` — the corresponding pass implementation when
     it doesn't fit alongside the driver.
   - `emit` — diagnostic-emission helpers, one per violation shape.
   - `queue` — the `PendingViolation` payload for rules that split
     across pre-expansion and late passes.
   - `scan` / `parser` — source-text walkers and parser combinators.
   - `ordering` / `triviality` — rule-specific algorithms.

   The `macro_argument_binding/`, `macro_trailing_comma/`,
   `prefer_raw_string/`, `derive_ordering/`,
   `unicode_ellipsis_in_panic_messages/`, and
   `single_letter_closure_param/` directories illustrate the
   pattern.

4. **Cross-rule helpers are `pub(crate)`, not `pub`.** The crate is
   a dylint `cdylib` with no public API surface, so `pub`
   over-advertises. Items in `src/common.rs`, `src/macro_path.rs`,
   `src/enclosing_hir.rs`, and `src/literal_scan.rs` should all be
   `pub(crate)` (or tighter). Use `pub(super)` for items that are
   only meant to leak one module level up — e.g. a rule's `Config`
   struct that's read by the rule's flat `.rs` driver but nowhere
   else.

   When deciding between `src/common.rs` and a dedicated module:
   `common.rs` is for short, self-contained, single-concept
   helpers (`is_single_ascii_letter`, `binding_ident`). Anything
   that has its own invariants worth documenting in a module
   docstring — a generic HIR walker, a per-character emit loop, a
   path-set parser — earns its own file.

## When the implementation is complete

If a PR fully implements a rule — every sub-check, every
configuration knob, every documented autofix — the planning file
becomes documentation drift. Remove it:

1. **Delete the rule's markdown file** from `planned-rules/`.
2. **Update `planned-rules/README.md`**: remove the rule's index
   entry. The "Out of scope" section at the bottom doesn't list
   implemented rules, only ones that won't be implemented; don't
   move the entry there.
3. **Fix every link and prose reference** to the deleted rule.
   Cross-references typically appear in:
   - Other rules' "Interaction with sibling lints" sections.
   - `planned-rules/IMPLEMENTATION_CONVENTIONS.md`, when the rule
     was an example of a convention.
   - The README's index, beyond just the entry being removed —
     some entries describe one rule by reference to another.

   Fix each reference by either pointing at the implementation
   source code (e.g., `src/qualified_paths.rs`) or rewording the
   prose to drop the link entirely. A reference that just names
   the rule for context can be reworded to use the lint's
   namespaced name (`perfectionist::qualified_paths`) without a
   markdown link.

After the cleanup, the repository should be self-consistent: the
`planned-rules/<rule>.md` file no longer exists, and no other
file points at it. Run `grep -r '<rule-name>' planned-rules/` to
confirm.

## When the implementation is partial

If a PR implements only some of a rule's sub-checks, configuration
knobs, or autofix branches, the planning file stays:

1. **Update `planned-rules/<rule-name>.md`** to reflect what's
   still pending. The simplest approach is a "Status" section
   near the top that lists what's implemented vs. what's not. The
   rest of the file remains the active spec for the unimplemented
   portion.
2. **Update the README index entry** if the rule's user-visible
   scope changed. If a sub-check or style was renamed during
   implementation, the index entry should match the implemented
   form.
3. **Cross-references usually stay as-is.** They pointed at the
   rule, and the rule still exists. The exception: a cross-
   reference that named a specific sub-check or knob that has
   since been removed in the implementation; update the reference
   to point at the still-present portion or drop it.

The rule file lives until everything in it is implemented or
explicitly retracted.

## Registering a new rule in `lib.rs`

Every rule module exposes two registration functions:

- `register_lint(lint_store)` — registers the lint declaration
  only.
- `register_pass(lint_store)` — installs the rule's early/late
  pass.

`src/lib.rs::register_lints` calls them in two phases: every
`register_lint` first, then every `register_pass`. The phasing
exists because `unknown_perfectionist_lints::register_pass`
snapshots the registered `perfectionist::*` lint names out of the
`LintStore` at construction time, so every rule's lint
declaration must already be in the store before any pass is
installed.

When you add a new rule:

1. Add the `mod` line and expose both `register_lint` and
   `register_pass` from the rule module.
2. Call `your_rule::register_lint(lint_store)` in the phase-1
   block of `register_lints` and
   `your_rule::register_pass(lint_store)` in the phase-2 block.
3. Do not introduce a parallel `REGISTERED_LINT_NAMES`-style
   array. The `LintStore` is the single source of truth.

## Validating Rust changes

After modifying any Rust code in this repository, install the
necessary developer tools and run every task that `just all`
performs. The recipe lives in the top-level `justfile`.
The toolchain — `cargo-dylint`, `dylint-link`, and
anything else the project requires — is provisioned by
`just install-dev-tools`, which drops binaries into
`.dev-tools/bin` (already on `PATH` via the justfile). Run it
once per fresh checkout, and again whenever `Cargo.lock` updates
the pinned `dylint_linting` version.

Treat `just all` as the gate before committing. Don't rely on
running a single sub-recipe (e.g. only `cargo test`) — the
`self-lint` step runs perfectionist's own lints on its source and
catches violations the other steps miss.

If `just install-dev-tools` cannot complete for any reason (an
extremely rare failure — sandboxed environments without network
access, an upstream registry outage, an incompatible host
toolchain), the workspace is not just missing `cargo-dylint`:
`.cargo/config.toml` also pins `dylint-link` as the linker, so
every cargo-backed `just all` step (`build`, `doc`, `lint`,
`test`) fails to link until it's present. In that case, override
the linker for each cargo invocation the same way
`install-dev-tools` does internally — pass
`--config 'target."cfg(all())".linker="cc"'` to `cargo` — and
skip the `self-lint` step entirely, since there's no
`cargo-dylint` to drive it. Then read the in-tree rule catalogue
under `rules/` and apply the rules manually to whatever code you
just wrote or touched. Each per-rule markdown file in `rules/`
is the human-readable spec for one lint; walk the diff against
the relevant rules and fix violations by hand. Note this
fallback explicitly in your summary so the user knows the
automated self-lint did not run.

## Generated documentation site (`tools/gen-docs/`)

The lint catalogue at <https://ksxgithub.github.io/perfectionist/>
is rendered by `tools/gen-docs/` into a single, self-contained
`gh-pages/index.html`. Two preferences shape what may go on the
page:

- **CSS over JavaScript.** Reach for CSS first; only add an inline
  `<script>` when CSS genuinely can't express the behaviour. When
  a script is necessary, render the JS-controlled element with the
  HTML `hidden` attribute and have the script clear it once its
  handlers are wired up — that covers every "JS isn't running"
  mode (browser scripting disabled, CSP-blocked inline scripts,
  ad-blocker-stripped tags, parse errors before the handler
  attaches). `<noscript>` only catches the "scripting disabled"
  case and leaves the others as visible-but-inert UI; reach for it
  only when the JS is pure enhancement (the page already works
  without it).

  One CSS subtlety to keep in mind: the UA stylesheet expresses
  `hidden` as `[hidden] { display: none }`, but author-origin
  rules beat UA-origin rules at equal specificity. Any author
  `display` declaration on the JS-controlled element will silently
  override the UA rule and the element will stay visible despite
  `hidden`. Pair the pattern with a sweeping
  `[hidden] { display: none !important }` reset (already in
  `tools/gen-docs/src/style/base.css`) to make `hidden` unconditional,
  or qualify every author `display` declaration with
  `:not([hidden])`. The reset is the standard fix and is shipped
  by Bootstrap / Tailwind / most CSS resets.

- **Conservative with bleeding-edge CSS.** Browser versions in
  active use trail current Baseline by several years. Before
  relying on a recent feature (`view-timeline`, `@container`,
  `:has()`, scroll-driven animations, etc.), check caniuse against
  a several-year-old cutoff and provide a polyfill-free fallback
  for older Firefox / Safari / mobile-Chromium engines — not just
  `@supports`-gated paths, since those still leave older browsers
  feature-less.

### Verifying rendering changes

`just all` doesn't cover layout, so a visual change can slip in a
regression that no automated step catches. Humans verify by opening
`gh-pages/index.html` in a browser. Agents that can't do that should
drive a headless Chromium from a throwaway Playwright script:

- Use Playwright's Chromium headlessly. If the environment
  already has Playwright and its Chromium binary available, point
  the launcher at them; otherwise install Playwright into a
  temporary location — the standard
  `npm i -D playwright && npx playwright install chromium` works —
  and use that. Write a one-off `.mjs` that imports `chromium`
  from `"playwright"`, launches it, loads `gh-pages/index.html` at
  the viewport sizes the change cares about, and dumps screenshots
  and/or `getBoundingClientRect` / `getComputedStyle` values.

- Treat the script as scratch. Don't commit it (and don't add it
  to `.gitignore` either — local exclusions belong in
  `.git/info/exclude`). The catalogue doesn't need a permanent
  test harness; ad-hoc checks for a specific change are easier to
  write fresh than to maintain.

- Chromium only. For cross-engine concerns (Firefox / Safari
  support of a CSS feature), pair the screenshots with a caniuse
  check — Chromium support is not proof a feature is safe to ship.

## Rationale section: "Why is this bad?" vs "Why restrict this?"

Every rule's documentation — both the rustdoc on the
`declare_tool_lint!` block and the planning file in
`planned-rules/` — needs a section that explains the motivation.
Pick the heading based on whether the violation is objectively
broken or a stylistic preference:

- **"Why is this bad?"** — use only when violating the rule
  produces an objective defect: silently broken behaviour, a
  user-visible rendering bug, a typo that neutralises a
  suppression, data corruption, a security issue, or similar.
  The reader should be able to read the section and agree that
  the practice is wrong on its own terms, not just out of taste.
  Examples in this repository:
  `perfectionist::unknown_perfectionist_lints` (a typo in
  `#[allow(perfectionist::...)]` silently fails to suppress
  anything) and `planned-rules/clap-help-no-markdown.md`
  (markdown leaks into the terminal `--help` output as literal
  syntax).
- **"Why restrict this?"** — use for every other rule. Most lints
  in this catalogue enforce stylistic preferences (em-dash usage,
  ellipsis form, derive choice, module layout, doc-comment
  length, etc.). The practice they forbid is not wrong in any
  absolute sense; the project simply prefers the alternative.
  Open these sections with an explicit disclaimer such as "This
  is a stylistic preference, not a correctness issue.", then
  explain the preference.

Do not use "Why is this bad?" as a default heading. Claiming
objective badness for what is really a preference misleads
readers and invites pushback that the rule's rationale cannot
sustain. When in doubt, the rule is a preference; use "Why
restrict this?".

Headings like "Why both lints together", "Why one rule instead
of two", "Why not `format!`-family", etc. are *design*
rationale (explaining the rule's shape, not the user practice it
forbids). Those are independent of this convention and may
coexist with either of the headings above.

## Notes on cross-rule dependencies

A handful of rules share helpers — the markdown exclusion
scanner, the unicode-width helper, the format-template parser,
the URL-discovery scanner. The planning files document who
depends on whom in the "Interaction with sibling rules"
sections. When implementing the *first* rule in a dependency
cluster, factor the shared helper into a crate-internal module so
the second rule can reuse it. The planning files name this
expectation explicitly; don't duplicate the helper.

## Commit message style

Every commit in this repository follows the
[Conventional Commits](https://www.conventionalcommits.org/)
style — a `type(scope): subject` header with the type drawn from
the usual set (`feat`, `fix`, `docs`, `chore`, `ci`, `refactor`,
`style`, `build`, `test`, `perf`), an optional scope in
parentheses, and a `!` after the type/scope for breaking changes
(e.g. `feat!: remove ...`). New commits you author should match.

The single exception is version-bump commits, whose subject is
just the version itself (e.g. `0.0.0-rc.6`). Use this form only
for commits that do nothing other than bump the version.

## Markdown links

`README.md` uses absolute links throughout, because it is
rendered by multiple services (crates.io, docs.rs, lib.rs, etc.)
that don't share a common base for resolving relative paths. Keep
new links there absolute.

Every other markdown file in the repository — `CLAUDE.md`,
`planned-rules/*.md`, and anything else committed alongside the
source — is only rendered in contexts where relative paths
resolve correctly (GitHub, local editors, agent tooling). Prefer
relative links in those files; they survive repository renames
and don't bake in a hosting URL.

## Symlinks

This file is the authoritative implementation guide. It is also
exposed under two other names so that other AI assistants and
agent harnesses pick it up automatically:

- `AGENTS.md` (symlink to `CLAUDE.md`).
- `.github/copilot-instructions.md` (symlink to `CLAUDE.md`).

Edit `CLAUDE.md` only; the symlinks pick up the change.
