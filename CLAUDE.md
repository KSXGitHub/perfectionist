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
   own file today.)

2. **When writing code, give each rule its own file and `Config`.**
   The file name matches the rule name (snake_case, no
   `perfectionist::` prefix). `CONFIG_KEY` is the full namespaced
   name (`perfectionist::<rule_name>`). The `Config` struct holds
   only the fields the rule actually reads — fields nominally
   "about" the rule but consumed by a different rule belong in
   that other rule's `Config`. If two rules genuinely share a
   helper function or type, factor it into `src/rules/common.rs`
   (for trivial cross-rule utilities) or a dedicated
   crate-internal module rather than co-housing the rules in one
   file.

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

## Symlinks

This file is the authoritative implementation guide. It is also
exposed under two other names so that other AI assistants and
agent harnesses pick it up automatically:

- `AGENTS.md` (symlink to `CLAUDE.md`).
- `.github/copilot-instructions.md` (symlink to `CLAUDE.md`).

Edit `CLAUDE.md` only; the symlinks pick up the change.
