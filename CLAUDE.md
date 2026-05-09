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
     `clippy_utils::declare_tool_lint!`. The planning files use
     the unqualified form (`qualified_paths`) for readability;
     the registered name is `perfectionist::qualified_paths`.

If the rule is one of several that share a helper (markdown
exclusion, format-string parsing, URL discovery, unicode-width
measurement), check whether the helper already exists in the
codebase before writing a new one. Sibling-rule references in the
planning files identify shared infrastructure.

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

## Notes on cross-rule dependencies

A handful of rules share helpers — the markdown exclusion
scanner, the unicode-width helper, the format-template parser,
the URL-discovery scanner. The planning files document who
depends on whom in the "Interaction with sibling rules"
sections. When implementing the *first* rule in a dependency
cluster, factor the shared helper into a crate-internal module so
the second rule can reuse it. The planning files name this
expectation explicitly; don't duplicate the helper.

## Symlinks

This file is the authoritative implementation guide. It is also
exposed under two other names so that other AI assistants and
agent harnesses pick it up automatically:

- `AGENTS.md` (symlink to `CLAUDE.md`).
- `.github/copilot-instructions.md` (symlink to `CLAUDE.md`).

Edit `CLAUDE.md` only; the symlinks pick up the change.
