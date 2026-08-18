# Implementing rules from `planned-rules/`

This repository's lint rules are documented in `planned-rules/`
before they are implemented. Each markdown file describes one
rule's intent, configuration knobs, examples, implementation
notes, and difficulty. Cross-cutting conventions (parser style,
lint-name namespacing, proc-macro-synthesis suppression) live in
`planned-rules/IMPLEMENTATION_CONVENTIONS.md`.

This guide tells you how to implement those rules and how to
keep the catalogue in sync as you go.

## Before you write code

Read these first, in this order:

1. **The rule's own file**, `planned-rules/<rule-name>.md`. It
   specifies the lint's identifier, configuration, the precise
   trigger predicate ("What to lint"), suggested fixes, and an
   estimated difficulty. Don't second-guess the design without
   first checking the rule file's existing examples and rationale
   — the design has usually been argued over already in the PR
   that produced the planning file.
2. **`planned-rules/README.md`** — the index of all rules, plus
   the
   [out-of-scope list](planned-rules/README.md#out-of-scope-cannot-be-linted-by-dylint)
   at the bottom. The index entry is the one-sentence summary;
   check that the rule you're implementing still says what you
   think it says.
3. **`planned-rules/IMPLEMENTATION_CONVENTIONS.md`** — applies
   to every rule. Among the cross-cutting conventions it covers:
   - **Parser style.** Non-trivial string scanners (URLs,
     emails, format templates, markdown spans, serde-attribute
     type literals) are written as parser-combinator-style
     `take_*` functions, not regex.
   - **Naming a lint after the anti-pattern.** A lint is named for
     the violation it fires on — never the fix, the remedy, or the
     preference — so it reads correctly under `#[deny(...)]` and
     `#[allow(...)]`. Follow Clippy's idiom, and mirror a Clippy
     lint's name only when the rule is a genuine *refinement* of it
     (not a contradiction or a complement). The name must claim no
     more than the trigger checks.
   - **Lint name namespacing.** Every lint registers under the
     `perfectionist` tool namespace via
     `rustc_session::declare_tool_lint!`. The planning files use
     the unqualified form (`path_qualification_mismatch`) for
     readability; the registered name is
     `perfectionist::path_qualification_mismatch`.

If the rule is one of several that share a helper — markdown
exclusion, format-string parsing, URL discovery and unicode-width
measurement are examples, not the whole set — check whether the
helper already exists in the codebase before writing a new one.
Sibling-rule references in the planning files identify shared
infrastructure.

## One rule per file, one `Config` per rule

The catalogue is organised so that each rule has exactly one
source file at `src/rules/<rule_name>.rs` and exactly one `Config`
struct keyed by the rule's full namespaced name. The convention
has these consequences for the implementer:

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
   directory module beside the flat `.rs` entry.** This crate
   forbids the `mod.rs` form (via `clippy::mod_module_files`,
   enabled in `Cargo.toml`), so the layout is `src/rules/<rule>.rs` next to
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

   Any rule with a `src/rules/<rule>/` directory beside its flat
   `.rs` entry is an instance of the pattern.

4. **Cross-rule helpers are `pub(crate)`, not `pub`.** The crate is
   a dylint `cdylib` with no public API surface, so `pub`
   over-advertises. Items in the crate-internal helper modules —
   every `src/*.rs` beside `lib.rs` and `rules.rs` — should all be
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

## Do not write documentation that restates the code

Prose that duplicates a fact the code already states is unverified
by construction: nothing fails when the code changes and the prose
does not. Every such sentence is a future lie with a long fuse.
Before writing one, ask what would make it go stale, and prefer the
form that cannot.

The shapes to avoid:

- **Never introduce a list with its own length.** "The convention
  has two consequences", "Six rules scan a slice of markdown" — the
  number is a second copy of the list, and the copy is what rots.
  Write "The convention has these consequences".
- **Prefer a greppable predicate to a hand-maintained roster.**
  "Every rule that imports `crate::markdown`" stays true forever; a
  list of rule names does not. Where a roster genuinely aids
  comprehension, mark it as illustrative ("for example", "among
  them") so no reader mistakes it for exhaustive and nobody has to
  maintain it.
- **Do not quote another file's heading or prose verbatim.** Link
  to the section instead. A quoted heading is a copy that no tool
  checks; a quoted sentence is worse, because anyone may reword the
  original without knowing the copy exists.
- **State a fact in exactly one place.** Defaults, config shapes,
  and file paths have one home; everything else links to it. The
  [defaults convention](#defaults-live-in-field-docs-not-type-or-variant-docs)
  below is this rule applied to the case that recurs most.
- **Do not paste real code into a guide.** An example copied out of
  a live `Config` struct drifts the moment that struct changes.
  Write the example with obviously-fake names, or generate it.
- **Do not describe a lint as existing until it is registered.**
  Shipped docs — `declare_tool_lint!` rustdoc, plus the `rules/*.md`
  catalogue and docs site generated from it — must not name a lint
  that `src/lib.rs::register_lints` does not register: a reader who
  acts on it and writes `#[expect(perfectionist::<that name>)]` is
  flagged by `perfectionist::unknown_perfectionist_lints`. Planning
  files under `planned-rules/` are exempt — naming unimplemented
  siblings is what they are for.
- **If a passage exists only to restate the code beneath it, delete
  it.** Length is not thoroughness. The shortest documentation that
  is still true next quarter beats the most complete documentation
  that is wrong.

None of this forbids duplication that genuinely has to exist. A
built-in default list restated in a config field's doc is the only
copy a user can read, and the `RuleConfig` mirrors in `tests/` are
there because the test crate cannot link the `cdylib`. Keep those,
say in the prose why the copy exists, and claim no more for it than
it delivers — a mirror that covers part of a rule's configuration
should not describe itself as the whole shape.

The mechanical half of this is worth a grep before you commit a
documentation change: a backticked `src/…`, `tests/…`, `ui/…`,
`rules/…` or `planned-rules/…` path should resolve, and a
`perfectionist::<name>` should name a lint that
`src/lib.rs::register_lints` registers. Both greps have standing
exceptions — prose describing a *linted* crate names paths that do
not exist here, planning files and this guide name rules that are
not implemented yet, and the `unknown_perfectionist_lints`
fixtures and `gen-docs`' unit tests use deliberately fake lint
names — so read the hits rather than the count.

## Defaults live in field docs, not type or variant docs

A config field's default value is documented on the **field**, never
on the field's **type** or on an **enum variant**. The default is a
property of the field, not of the type: the same type could back
another field with a different default, so a default stated on the
type (or one of its variants) is misattributed and goes stale the
moment a second field reuses it. The field doc is also where readers
look — `tools/gen-docs/` renders it under "Configuration", the
section a reader scans for defaults.

So a field doc states the default in the config-file value form, and
the type / variant docs describe only what each value *means*:

```rust
/// How the frobnicator is drawn. Defaults to `rounded`.
frob_style: FrobStyle,               // field doc carries the default

enum FrobStyle {
    /// Every corner is left at a right angle.
    Square,
    /// Every corner is filleted to the configured radius.
    Rounded,                     // variant doc: meaning only, no "default"
}
```

The names above are deliberately fake: an example copied out of a
live `Config` struct would itself be a restatement of the code, and
would drift the moment that struct changed.

The wrong placement — `Rounded`'s doc reading "… the default." or
`FrobStyle`'s own doc reading "Defaults to `rounded`." — is the
recurring mistake this convention exists to stop. It already had to
be corrected once for `ReferenceScope::Crate` in
<https://github.com/KSXGitHub/perfectionist/pull/218>. The
`#[default]` *attribute* on a variant is fine — it is code
expressing the `Default` impl, not prose claiming a default. Only
the prose is governed here.

## When the implementation is complete

If a PR fully implements a rule — every sub-check, every
configuration knob, every documented autofix — the planning file
becomes documentation drift. Remove it:

1. **Delete the rule's markdown file** from `planned-rules/`.
2. **Update `planned-rules/README.md`**: remove the rule's index
   entry. The
   [out-of-scope list](planned-rules/README.md#out-of-scope-cannot-be-linted-by-dylint)
   at the bottom doesn't hold implemented rules, only ones that
   won't be implemented; don't move the entry there.
3. **Fix every link and prose reference** to the deleted rule.
   Cross-references typically appear in:
   - Other rules' "Interaction with sibling rules" sections.
   - `planned-rules/IMPLEMENTATION_CONVENTIONS.md`, when the rule
     was an example of a convention.
   - The README's index, beyond just the entry being removed —
     some entries describe one rule by reference to another.

   Fix each reference by either pointing at the implementation
   source code (`src/rules/<rule_name>.rs`) or rewording the prose
   to drop the link entirely. A reference that just names the rule
   for context can be reworded to use the lint's namespaced name
   (`perfectionist::<rule_name>`) without a markdown link.

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

- Treat the script as scratch, and keep it out of the repository
  entirely. There are three ways to write temporary files: The first
  is to write them to directories outside the project directory
  (such as `/tmp`). The second is to create a directory named `tmp/`
  and put all temporary files in it. The third is to name the file
  in the `tmp.*` pattern.

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
  anything) and `perfectionist::clap_help_markdown`
  (`src/rules/clap_help_markdown.rs`) (markdown leaks into the
  terminal `--help` output as literal syntax).
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

Rules share crate-internal helpers: every `src/*.rs` beside
`lib.rs` and `rules.rs` is one, and each carries a module
docstring saying what it is for. The module-re-parsing helper
(`src/module_reparse.rs`) is the one to know about before you
start — it re-parses the crate's module source files from a
shared `SourceMap` so a rule reaching separate-file submodules
still sees `#[cfg(...)]` gates intact. It exists because this
exact bug — a source-layout rule shipped as a pre-expansion
`EarlyLintPass` silently skipping every separate-file submodule —
has been written twice; before implementing any rule that reads
the *written layout* of items across module scopes, read
[Reaching every module (source-layout rules)](planned-rules/IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules).
The planning files document who depends on whom in their
"Interaction with sibling rules" sections. When implementing the
*first* rule in a dependency cluster, factor the shared helper
into a crate-internal module so the second rule can reuse it. The
planning files name this expectation explicitly; don't duplicate
the helper.

One shared helper is a suppression guard rather than a parser:
`crate::common::hir_in_external_macro` (late passes) and
`clippy_utils::is_from_proc_macro` (early passes) keep rules from
false-positiving on proc-macro-synthesised nodes that carry a
user-source span and so slip past `report_in_external_macro:
false`. This bug class has recurred across rules; before you pick
a new rule's diagnostic span, read
[Suppressing proc-macro-synthesised violations](planned-rules/IMPLEMENTATION_CONVENTIONS.md#suppressing-proc-macro-synthesised-violations)
and add the guard plus a `ui/<rule>_proc_macro.rs` regression
fixture if the rule is vulnerable. The fixture is only real if it
fails with the guard removed: build it around a trigger the rule
actually fires on (not an exempt or trivial node), and
mutation-check it before trusting it.

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

Keep the subject line (the first line) at **72 characters or
fewer** — the conventional hard cap; 50 is the ideal. GitHub's web
UI truncates a longer subject, appends a `…`, and folds the
overflow into the body behind another `…`, so the commit reads as a
mangled title/body split. The [`commit-msg`
hook](.githooks/commit-msg) rejects any commit whose subject
exceeds the cap (version-bump commits are exempt, but are short
anyway); it measures characters, not bytes, so an accented or
non-Latin subject is judged by what GitHub displays. Move any
detail that does not fit into the commit body — a blank line after
the subject, then the rest.

When a longer subject is genuinely warranted, set
`PERFECTIONIST_GIT_HOOK_ALLOW_LONG_SUBJECT=true` for that `git
commit` to skip just this check; the hook's other checks still
apply. `false`, empty, and unset all enforce the cap, and any other
value is rejected so a misspelt opt-out never silently fails to
apply.

## Issue and PR references in commit messages

Never write a **bare** `#NNN` reference in a commit message. A
reference is *bare* when a `#` followed by digits is not
immediately preceded by an ASCII word char (`[0-9A-Za-z_]`) — the
form GitHub autolinks against *this* repo, so a number meant for
another project links to the wrong issue. Thus `#123`, `(#123)`,
`path/#123` are bare; `owner/repo#123`, `C#123`, `foo#123` are
not. The [`commit-msg` hook](.githooks/commit-msg) rejects any
commit containing one; install it on a fresh checkout with
`just install-git-hooks` (or `git config core.hooksPath .githooks`).

Use an unambiguous form instead:

- **Same repo:** `KSXGitHub/perfectionist#123`
- **Another repo:** `owner/repo#123`
- **Absolute URL:** `https://github.com/owner/repo/issues/123`

To enumerate list items, number them without `#` ("item 1", "1.",
"(1)") — "`#1`"/"`#2`" otherwise autolink as issue 1/2. The hook
skips comment lines and the `git commit -v` diff, and ignores
non-bare forms like `#123abc` / `#L123` / `#issuecomment-1`.

## `@handle` mentions in commit messages

Never write a **bare** `@handle` in a commit message — always wrap it
in backticks (`` `@foo` ``). Outside backticks GitHub autolinks `@foo`
as a user/team mention, which is wrong both ways: if you meant it as
code (a `@scope/pkg`, a CLI flag), GitHub still reads it as a mention;
if you meant a real account, every commit, comment, and rebase that
carries the message pings that person again. The same
[`commit-msg` hook](.githooks/commit-msg) rejects any commit
containing one. It strips backtick-wrapped spans before scanning (so
`` `@foo` `` passes) and uses GitHub's boundary, so an email like
`user@example.com` is not flagged. A genuinely intended mention
belongs in the PR or issue body, not the commit message.

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

## GitHub-specific markdown

Some markdown renders as intended only on GitHub. Whether a file
may use it depends on who renders that file.

**Documentation that double-serves as rustdoc must not use it.**
These are rustdoc-bound:

- the rustdoc on a `declare_tool_lint!` block, and
- the generated `rules/*.md`, which `tools/gen-docs/` renders
  from that rustdoc, together with the docs site built from the
  same source.

`planned-rules/*.md`, `CLAUDE.md`, and the other in-repo guides
are only ever rendered by GitHub, local editors, and agent
tooling, so they may use these features where one genuinely
helps. `README.md` is the exception among them: `Cargo.toml` sets
`readme = "README.md"`, so it also ships to crates.io and lib.rs.
Keep it conservative for the same reason its links are absolute.

The constraint is not "GitHub invented it" but "the other two
renderers drop it". Rustdoc parses CommonMark plus a few
extensions, and `tools/gen-docs/` parses with `pulldown-cmark`
under `ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_FOOTNOTES`
(`tools/gen-docs/src/render/markdown.rs`). What survives each:

| Feature                   | rustdoc      | `gen-docs`   | GitHub   |
|---------------------------|--------------|--------------|----------|
| Alerts (`> [!NOTE]`)      | literal text | literal text | callout  |
| Task lists (`- [ ]`)      | checkbox     | literal text | checkbox |
| Mermaid fences            | code block   | code block   | diagram  |
| Tables, `~~del~~`, `[^1]` | renders      | renders      | renders  |

So the three to keep out of rustdoc-bound docs are **alerts, task
lists, and mermaid fences**. The alert is the trap worth naming:
its marker is left as literal text in both the in-tree catalogue
and the docs site, where it reads as a typo.

`<details>` / `<summary>` renders in all three, but still does
not belong in rustdoc-bound docs. `rules/*.md` exists so the
catalogue can be read in an editor without a browser, and
`tools/gen-docs/src/render_md.rs` drops the HTML renderer's
`<details>` panels for that reason; raw HTML in the markdown copy
works against its purpose.

### Choosing the alert type

GitHub defines five, and their plain meanings apply: `[!NOTE]`,
`[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]`. The one
editorial rule worth stating is that `[!WARNING]` and
`[!CAUTION]` mark an actual hazard — data loss, breakage, a
security issue — never emphasis. A spec caveat is not a hazard,
and reserving those two is what keeps them worth reading.

### A blockquote is not always an alert

`>` is also plain markdown for a quotation, and the catalogue
relies on that. A `>` block in `planned-rules/` is one of three
things, and only the second may become an alert:

- **A quotation.** Every `## Statement` section quotes the rule's
  upstream style-guide source verbatim. Leave these alone, and do
  not run a sweep that mechanically upgrades every `>` block to
  an alert.
- **An aside** — a remark in the author's own voice, addressed to
  the reader. This is the one an alert improves; pick its type by
  the rules above.
- **A sketch of emitted diagnostic text**, typically introduced
  by a line ending "should emit text along these lines:". Leave
  it as a blockquote: the markup inside is part of the sketch, so
  a `text` fence would render the `**error:**` markers literally
  and lose what the block is showing.

## Symlinks

This file is the authoritative implementation guide. It is also
exposed under two other names so that other AI assistants and
agent harnesses pick it up automatically:

- `AGENTS.md` (symlink to `CLAUDE.md`).
- `.github/copilot-instructions.md` (symlink to `CLAUDE.md`).

Edit `CLAUDE.md` only; the symlinks pick up the change.
