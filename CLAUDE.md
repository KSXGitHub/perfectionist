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
   first checking the rule file's existing examples and rationale.
   The design has usually been argued over already in the PR that
   produced the planning file. Weigh the design rather than
   transcribe it, though; see
   [A planning file is a proposal, not a specification](#a-planning-file-is-a-proposal-not-a-specification).
2. **`planned-rules/README.md`**, the index of all rules, plus
   the
   [out-of-scope list](planned-rules/README.md#out-of-scope-cannot-be-linted-by-dylint)
   at the bottom. The index entry is the one-sentence summary;
   check that the rule you're implementing still says what you
   think it says.
3. **`planned-rules/IMPLEMENTATION_CONVENTIONS.md`**, which applies
   to every rule. Among the cross-cutting conventions it covers:
   - **Parser style.** Non-trivial string scanners (URLs,
     emails, format templates, markdown spans, serde-attribute
     type literals) are hand-written recursive-descent parsers
     built from `take_*` functions, not regex. A `take_*` takes a
     prefix and hands back the remainder; the parse that owns the
     input requires it to be exhausted, which is the distinction
     the conventions file spells out.
   - **Naming a lint after the anti-pattern.** A lint is named for
     the violation it fires on, never the fix, the remedy or the
     preference, so that it reads correctly under `#[deny(...)]`
     and `#[allow(...)]`. Follow Clippy's idiom, and mirror a Clippy
     lint's name only when the rule is a genuine *refinement* of it
     (not a contradiction or a complement). The name must claim no
     more than the trigger checks.
   - **Lint name namespacing.** Every lint registers under the
     `perfectionist` tool namespace via
     `rustc_session::declare_tool_lint!`. A per-rule `dylint.toml`
     table is *addressed* by its config key, so its header has to
     be the namespaced name, quoted as
     `["perfectionist::path_qualification_mismatch"]`, in a
     planning file's `## Configuration` fence exactly as in a
     consumer's `dylint.toml`; `#[allow(...)]` / `#[deny(...)]`
     likewise take the `perfectionist::`-qualified path rustc
     resolves. Where the rule is merely *named*, the unqualified
     `path_qualification_mismatch` is correct, both in prose and in
     a `[perfectionist]` `enable` / `disable` entry. See the
     conventions file for the per-context spelling.

If the rule is one of several that share a helper, check whether
the helper already exists in the codebase before writing a new
one. Markdown exclusion, format-string parsing, URL discovery and
unicode-width measurement are examples, not the whole set.
Sibling-rule references in the planning files identify shared
infrastructure.

## A planning file is a proposal, not a specification

Nothing in a planning file has been compiled or run. Read it as a
proposal to evaluate, not a specification to fulfil.

- **Its claims about reality may be wrong.** A planning file asserts
  what a HIR node exposes, what a span covers, what `clippy_utils`
  already provides, which are examples rather than the whole set.
  Verify each such claim empirically before you build on it: write
  the fixture, dump the AST or HIR, read the upstream source. Where
  a claim cannot be verified that way, leave it unclaimed rather
  than restate it in a doc or a comment.
- **It is often over-ambitious.** Among the shapes that recur:
  config knobs nobody would ever set, sub-checks no real code
  violates, autofix branches that cost more than the rule they
  serve. Implement what earns its complexity and leave the rest
  out, recording the omission per
  [When the implementation is partial](#when-the-implementation-is-partial).

Where the empirical answer contradicts the planning file, the
empirical answer wins: never implement something you have shown
to be wrong. Say in the PR what you dropped or changed and why,
so the divergence reads as deliberate rather than as an oversight.

What a planning file *is* authoritative about is the intent: its
`## Statement`, and the anti-pattern the lint fires on.

## One rule per file, one `Config` per rule

The catalogue is organised so that each rule has exactly one
source file at `src/rules/<rule_name>.rs` and exactly one `Config`
struct keyed by the rule's full namespaced name. The convention
has these consequences for the implementer:

1. **Before writing code, check whether the rule is actually one
   rule.** A planning file that bundles several independently-
   triggered checks under one banner is usually better
   implemented as several rules. If the sub-checks can be cleanly
   separated, with distinct trigger predicates, disjoint
   configuration and no shared diagnostic, split the planning file
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
   only the fields the rule actually reads. A field nominally
   "about" the rule but consumed by a different rule belongs in
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
   `declare_tool_lint!` block, the `Register` impl, the
   `EarlyLintPass` / `LateLintPass` driver, and any process-wide
   state (`static PENDING_VIOLATIONS`, etc.). Common
   submodule names that have emerged:
   - `config`: `Config` struct, default lists, in-memory rule state.
   - `early` / `late`: the corresponding pass implementation when
     it doesn't fit alongside the driver.
   - `emit`: diagnostic-emission helpers, one per violation shape.
   - `queue`: the `PendingViolation` payload for rules that split
     across pre-expansion and late passes.
   - `scan` / `parser`: source-text walkers and `take_*` parsers.
   - `ordering` / `triviality`: rule-specific algorithms.

4. **Cross-rule helpers are `pub(crate)`, not `pub`.** The crate is
   a dylint `cdylib` with no public API surface, so `pub`
   over-advertises. Items in the crate-internal helper modules,
   meaning every `src/*.rs` beside `lib.rs` and `rules.rs`, should
   all be `pub(crate)` (or tighter). Use `pub(super)` for items that are
   only meant to leak one module level up, such as a rule's
   `Config` struct that's read by the rule's flat `.rs` driver but
   nowhere else.

   When deciding between `src/common.rs` and a dedicated module:
   `common.rs` is for short, self-contained, single-concept
   helpers (`is_single_ascii_letter`, `binding_ident`). Anything
   that has its own invariants worth documenting in a module
   docstring earns its own file. A generic HIR walker, a
   per-character emit loop and a path-set parser each do.

## Choose a collection by observable order

Both `BTreeSet` / `BTreeMap` and `HashSet` / `HashMap` appear here. What
decides between them is whether anything depends on the order entries
come back out in. Size does not decide it, and at these sites neither
does speed: built once and probed once per node, the two spellings
differ by nanoseconds beneath a HIR walk that costs orders of magnitude
more. A collection on a genuine hot path is a performance question, and
measuring is how to settle one. These sites are not that.

That order reaches the reader when it sequences diagnostics, orders a
suggestion, or lands in a generated file. There the ordered container is
necessary and not sufficient, because what it sorts by is the key's
`Ord`, and that varies across `rustc_span`'s key types. `Span` and
`BytePos` compare by position in the file, which is the order the reader
is already scanning in. `Symbol` compares by its slot in the interner,
which is worse than hashed: stable enough to freeze into a `.stderr`
fixture and pass review, still arbitrary to whoever reads the
diagnostic. Holding a `Symbol` and owing alphabetical order, sort by
`Symbol::as_str` as you emit.

Where no order reaches the reader the collection is a lookup table,
either spelling is correct, and rewriting a working one from either
container to the other is noise.

Sometimes the choice is already made. A key may not implement the
trait: rustc's `HirId` has `Hash` and `Eq` and no `Ord`. A dependency
may name the container in its own signature, as
[`build-fs-tree`](https://docs.rs/build-fs-tree)'s directory variant
does.

## Do not write documentation that restates the code

Prose that duplicates a fact the code already states is unverified
by construction: nothing fails when the code changes and the prose
does not. Before writing such a sentence, ask what would make it go
stale, and prefer the form that cannot.

The shapes to avoid:

- **A count over a list that can grow is a second copy of it.**
  "The convention has two consequences", "Six rules scan a slice of
  markdown": the number rots the moment the list gains a member.
  Write "The convention has these consequences".
- **Prefer a greppable predicate to a hand-maintained roster.**
  "Every rule that imports `crate::markdown`" stays true forever; a
  list of rule names does not. Where a roster genuinely helps, mark
  it illustrative ("for example", "among them").
- **Do not quote another file's heading or prose verbatim.** It is a
  copy no tool checks, and the original can be reworded by someone
  who never sees it. Naming a heading that the conventions require
  every file of a kind to carry is naming the convention, not copying
  a fact. In markdown,
  link to the section by anchor; in Rust, where a repo-relative link
  does not resolve, name the file and let the reader search it.
- **State a fact in exactly one place.** Defaults, config shapes and
  file paths have one home; everything else links to it. The
  [defaults convention](#defaults-live-in-field-docs-not-type-or-variant-docs)
  below is this rule applied to the case that recurs most.
- **Do not paste real code into a guide.** An example copied out of
  a live `Config` struct drifts the moment that struct changes.
  Write it with obviously-fake names.
- **Do not describe a lint as existing until it is registered.**
  Shipped docs must not name a lint that `src/rule_index.rs` does
  not, or a reader who writes
  `#[expect(perfectionist::<that name>)]` for it is flagged by
  `perfectionist::unknown_perfectionist_lints`. Planning files are
  exempt; naming unimplemented siblings is what they are for.
- **If a passage exists only to restate the code beneath it, delete
  it.** Length is not thoroughness.

Some copies have to exist: a config field's default list is the only
one a user can read, and the `RuleConfig` mirrors in `tests/`
serialise config the test crate cannot reach. Keep those, say why
they exist, and keep a default list complete, since a partial one
is useless.

Some of these are mechanically checkable and worth a grep before
committing:
backticked in-repo paths should resolve, and a `perfectionist::`
name should be one the `rule_index!` invocation names. Both have
standing exceptions, so read the hits rather than the count. Among
them are prose about a *linted* crate, planning files and this
guide, the deliberate typos around
`unknown_perfectionist_lints`, and `gen-docs`' unit tests, which
invent lint names.

## Repeat the word; do not vary it

A synonym signals a distinction. Where there is none, it sends the
reader looking for one. Use the same word for the same thing, however
often it recurs.

- The same word in rustdoc, in comments, and in fixtures. "measured
  on its own" in one place and "measured independently" in the next
  describe one behaviour and read as two.
- Spell a value the way the code spells it. The diagnostic says
  `nests 1 level deep`, so the comment says `1 level`, not
  `one level`.
- Count parameters as the function type counts them. `Result::ok` is
  `fn ok(self) -> Option<T>`: a **unary function**, not a nullary
  method. The Reference makes `self` its first parameter, and that is
  exactly why `filter_map(Result::ok)` type-checks. Prefer that
  reading. Where the method-position count is the one that matters,
  give both, because rustc gives both: `this method takes 0 arguments`
  at a call site, and `this function takes 1 argument` for the same
  item named through a path.

The
[fact-duplication rule](#do-not-write-documentation-that-restates-the-code)
is this one's complement: repeat words freely, never facts.

## Say what it does, not what it resembles

A metaphor borrowed from outside software states nothing the reader can
check. "The guard is load-bearing" claims that removing it breaks
something without saying what, and "the knob earns its keep" rates it
against a standard it never names. Both read as decisive and leave the
mechanism to be guessed. Name the mechanism and the sentence becomes
checkable: "without the guard, every macro expansion is flagged".

The shapes to avoid:

- **A metaphor from another field**, such as construction, finance or
  medicine: "load-bearing", "papers over", "baked in", "belt and
  braces". Software has a plainer word for each, usually a shorter one.
- **An unnamed standard**: "earns its keep", "pays for itself", "worth
  the complexity". Give the cost and the benefit, or drop the claim.
- **Anthropomorphism**: "the rule wants a `Symbol`", "the parser is
  happy with an empty remainder". An item does what it does; write
  that.
- **A vogue phrase**: "low-hanging fruit", "moves the needle", "at the
  end of the day". Each is filler that names nothing.

These are *not* the fault, and a sweep that rewrites them makes the
prose worse:

- **Software's own vocabulary**, however it began; among them pass,
  span, walk, visitor, hook, sweep, drift, gate, guard, ratchet and
  fixture. A reader of this codebase knows each, which is what makes it
  a term rather than a figure.
- **A figure the passage defines where it stands.**
- **A dead metaphor that is simply the word for the thing**: a span
  *covers* bytes, a file *holds* items, a doc *points at* a section.

A lint's diagnostic needs the plain word most, because rustc prints it
to a reader who has never seen this guide.

Where the
[fact-duplication rule](#do-not-write-documentation-that-restates-the-code)
asks what would make a sentence go stale, this one asks what would make
it false. A sentence with no answer says nothing.

## Every passage must read on its own

A sentence that takes its meaning from a neighbouring one breaks when
anything is inserted between them, and nothing fails when it does. The
reader rarely arrives in writing order either: they land on one fixture
case from a `grep` hit, one hunk of a diff, or one rule's file in the
in-tree catalogue, where `rules/<rule_name>.md` carries that rule and
nothing else.

Two shapes carry the fault:

- **A positional reference**: "the case below", "`configure` above",
  "the fixtures above", "the same shape spelled as a tuple". Insert an
  item and it points at the wrong one. A name is greppable and survives
  reordering, so "the counterpart of `into_extended`" beats "the
  counterpart of the above"; where the target is in another file, name
  the file.
- **A dangling comparative**: a bare "likewise", "the same", "too",
  "either", "as well" or "ditto" whose antecedent sits in another item
  rather than in the passage holding it. Restate what it borrowed;
  a clause repeated costs less than the reference did.

These are *not* the fault, and a sweep that rewrites them makes the
prose worse:

- **A back-reference inside the same passage.** "two bindings of the
  same path", "not `Copy` either". The antecedent is in the sentence
  or the one before it, so the reader never leaves.
- **A threshold.** "one above the default limit of 10", "exactly 15 is
  not above the limit". Here "above" is the number, not a position.
- **Source order that is the subject.** "a `pub use` below a private
  import" is the violation `arbitrary_source_item_ordering` reports.
- **Prose introducing what immediately follows.**
- **An idiom.** "all the same", "either way", "in the same boat".

The `// Bad:` / `// Good:` / `// Not flagged:` labels under `ui/`,
`ui-toml/` and `tests/fixtures/` are where this bites hardest: each is
written beside its sibling and then read alone. Sweep them by searching
those directories for a label line that also carries "the same", "too",
"either", "as well", "likewise", "ditto", "above" or "below". Anchor the
search on the label itself, or the output fills with the exceptions
above. Anchor it on the label's word alone, though, since what follows it
varies: a colon in most files, a comma or another mark elsewhere, and
sometimes nothing at all. An anchor that assumes one separator silently
skips every file written with another. Then read each matching comment
whole, because a label's
continuation lines carry the fault as often as its first line does and a
line-oriented search returns only the line it matched.

Judge every hit; never count them. "Above" and "below" are the noisiest,
most of their hits being a threshold or the source order a rule reports.
"The same" and "too" are the offenders you will meet most, and are also
often innocent, so weigh each one. "Likewise" and "ditto" are rare and
nearly always the fault; "as well" is usually a back-reference inside
its own comment.

One trap is specific to a fixture: its comment may be the very text a
rule lints (`bare_issue_reference` scans plain line comments), and the
`.stderr` quotes the whole source line, so rewording the comment moves
the fixture's expected output with it.

This rule applies the
[fact-duplication rule](#do-not-write-documentation-that-restates-the-code)'s
test, which is to ask what would make it go stale, to a reference
rather than to a copied fact.

## Shipped docs address the consumer, not the contributor

A doc is *shipped* if a consumer reads it without cloning:
`README.md`, `CONFIGURATION.md`, `CONTROLLING_RULES.md`, and
everything `tools/gen-docs/` renders, among them a rule's
`declare_tool_lint!` rustdoc, its `Config` field docs, and the docs
on the types and variants those fields name. Everything else is
contributor-only and may say whatever is useful: `planned-rules/`,
this guide, `//!` docs, docs on items no `Config` field exposes,
`tests/`, `tools/`.

A shipped doc may name only what a consumer can reach: their code,
their `dylint.toml`, a registered lint, rustc, Clippy, a published
crate. The recurring violations:

- **A planning file.** It describes work that may never ship, in a
  directory the reader does not have.
- **Unshipped behaviour.** "... is still pending" is the backlog, not
  the lint. Describe the trigger the rule *has*.
- **A private Rust item.** `` [`Type::method`] `` is a valid rustdoc
  link, so no `cargo doc` run flags it. But `gen-docs` has no
  resolver, and the catalogue and the site render the brackets
  literally, where it reads as a typo. State the behaviour instead.
- **A project the rule was distilled from.** A consumer has never
  heard of it, so `matches <project>'s policy` says nothing about what
  a knob does, and "both source projects" has no antecedent at all.
  Say what the value means.
- **Development tooling.** A `just` recipe, a CI workflow, a dev
  dependency. The consumer runs `cargo dylint`.

Exceptions, where the reader chose the detour: `README.md` switches
audience by heading (its development section names `just` recipes, its
rules section offers `planned-rules/` as the roadmap), and a generated
file's do-not-edit banner names `just gen-rules-md` because it
addresses whoever is about to hand-edit it.

## A published crate is linked at its first shipped mention

The first code span naming a published crate in a rule's shipped docs
links to that crate's docs.rs root: `https://docs.rs/<crates.io name>`,
never an item page, which a rename or a module move breaks. The
crates.io spelling stands even where the link text is the underscored
Rust path, as in `` [`command_extra::CommandExtra`](https://docs.rs/command-extra) ``.

Later mentions of that crate on the same rendered page are plain, so a
crate the `declare_tool_lint!` rustdoc links is plain in the
`Configuration` section below it, and a crate only a `Config` field
names is linked there. The lint's one-line description is plain
wherever it falls: rustc prints it in a diagnostic, where a link is
literal text.

A code span names a crate when the crate is its leading path
segment: `` `thiserror` ``, `` `clap::Parser` ``, `` `log::info!` ``. An
attribute span (`#[serde(...)]`), a bare macro name (`quote!`), a code
fence, the standard library, and the rustc / Clippy / rustdoc lint
namespaces name something else and stay plain.

## Test documentation goes above the test

A comment describing a whole test is documentation, so it goes where
documentation goes: a `///` block above the `#[test]` attribute. A `//`
comment inside the body belongs to one line or one region of lines, and
a leading one that covers every line in the body is documentation in
disguise.

The split follows what each reader needs. The `///` block says what the
test pins and why the case is worth pinning, which is what someone
scanning the module reads. A `//` comment says why the line under it is
written that way, which only that line's reader needs. A test whose
body carries a series of comments, one per case, keeps them where they
are. Where a test has both a purpose and per-case notes, the purpose
moves up and the notes stay put.

A comment in disguise is already wrong where it sits, so leaving it is
not one of the options. Which fix applies is decided by what it says,
not by where it sits: a block that says what the test pins moves above
the attribute, and a block that narrates the body goes, per
[Do not write documentation that restates the code](#do-not-write-documentation-that-restates-the-code).
Moving launders nothing. Text that would read as a poor doc above the
attribute was a poor comment below it, and a test whose name already
says what it pins needs no block at all.

One fault follows the move itself. A later comment may have leaned on
the one now above the attribute, so restate what it borrowed, per
[Every passage must read on its own](#every-passage-must-read-on-its-own).

## Defaults live in field docs, not type or variant docs

A config field's default value is documented on the **field**, never
on the field's **type** or on an **enum variant**. The default is a
property of the field, not of the type: the same type could back
another field with a different default, so a default stated on the
type (or one of its variants) is misattributed and goes stale the
moment a second field reuses it. The field doc is also where readers
look, since `tools/gen-docs/` renders it under "Configuration",
the section a reader scans for defaults.

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

The wrong placement is the recurring mistake this convention
exists to stop: `Rounded`'s doc reading "… the default.", or
`FrobStyle`'s own doc reading "Defaults to `rounded`." The mistake
already had to be corrected once for `ReferenceScope::Crate` in
<https://github.com/KSXGitHub/perfectionist/pull/218>. The
`#[default]` *attribute* on a variant is fine, being code that
expresses the `Default` impl rather than prose claiming a default. Only
the prose is governed here.

## Re-read new prose before committing

Prose that breaks this guide's documentation conventions still
compiles, lints clean and passes the fixtures, so no step of
`just all` will catch it. You are the only check it has, and
writing a passage is a separate pass from checking one.

So before you commit, re-read every doc comment, guide sentence and
fixture label the diff adds or touches against the conventions that
govern it, and fix what fails; where a convention names a grep or a
sweep, run it. A passage written with a convention in mind still has
to be read against it.

## When the implementation is complete

If a PR fully implements a rule, every sub-check and configuration
knob and documented autofix included, the planning file becomes
documentation drift. Remove it:

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
   - The README's index, beyond just the entry being removed,
     since some entries describe one rule by reference to another.

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

## Registering a new rule in the index

`src/rule_index.rs` indexes every rule the plugin ships: its
`rule_index!` invocation names each rule once.

The list is alphabetical.

When you add a new rule:

1. Add the `pub mod` line to `src/rules.rs`.
2. Add an entry to the `rule_index!` invocation in
   `src/rule_index.rs`.
3. Implement `Register` for the type that entry generates, in the
   rule's own file.
4. Do not introduce a second list of rule names; the `rule_index!`
   invocation is the single source of truth.

## Validating Rust changes

After modifying any Rust code in this repository, install the
necessary developer tools and run every task that `just all`
performs. The recipe lives in the top-level `justfile`.
The toolchain is provisioned by `just install-dev-tools`, which
covers `cargo-dylint`, `dylint-link` and anything else the project
requires. It drops the binaries into
`.dev-tools/bin` (already on `PATH` via the justfile). Run it
once per fresh checkout, and again whenever `Cargo.lock` updates
the pinned `dylint_linting` version.

Treat `just all` as the gate before committing. Don't rely on
running a single sub-recipe (e.g. only `cargo test`). The
`self-lint` step runs perfectionist's own lints on its source and
catches violations the other steps miss.

If `just install-dev-tools` cannot complete for any reason (an
extremely rare failure, such as a sandboxed environment without
network access, an upstream registry outage or an incompatible
host toolchain), the workspace is not just missing `cargo-dylint`:
`.cargo/config.toml` also pins `dylint-link` as the linker, so
every cargo-backed `just all` step (`build`, `doc`, `lint`,
`test`) fails to link until it's present. In that case, override
the linker for each cargo invocation the same way
`install-dev-tools` does internally, by passing
`--config 'target."cfg(all())".linker="cc"'` to `cargo`, and
skip the `self-lint` step entirely, since there's no
`cargo-dylint` to drive it. Then read the in-tree rule catalogue
under `rules/` and apply the rules manually to whatever code you
just wrote or touched. Each per-rule markdown file in `rules/`
is the human-readable spec for one lint; walk the diff against
the relevant rules and fix violations by hand. Note this
fallback explicitly in your summary so the user knows the
automated self-lint did not run.

## The docs-site JavaScript

The JavaScript under `tools/gen-docs/` is formatted by
[oxfmt](https://oxc.rs/docs/guide/usage/formatter.html) and linted by
[oxlint](https://oxc.rs/docs/guide/usage/linter.html), both pinned in
[`package.json`](package.json) and configured in
[`.oxfmtrc.json`](.oxfmtrc.json) and
[`.oxlintrc.json`](.oxlintrc.json). After touching any of it, run
`just check-js`. `just fix-js` applies what oxlint can fix and then
reformats, in that order because a fix lands as valid code rather than
as formatted code.

## The maud templates

rustfmt does not format the body of a `{`-delimited macro invocation,
so `just fmt` leaves the layout inside a `maud::html!` body alone.
[maudfmt](https://docs.rs/maudfmt) settles it, pinned as a
dev-dependency in [`Cargo.toml`](Cargo.toml), and
[`tests/maud_formatting.rs`](tests/maud_formatting.rs) holds every
template to its output.

## Normalised `.stderr` fixtures

A `.stderr` under `ui/` or `ui-toml/` is a normalised copy of the
driver's output rather than the output itself: a span header's
`line:column` reads `LL:CC`, so inserting a line above a diagnostic
does not renumber every header below it. Spell it that way when
writing one, since pasting the driver's real numbers in will not match, or
let [`just bless`](#re-recording-a-stderr) record it.
[`utils/src/ui_fixtures.rs`](utils/src/ui_fixtures.rs)
performs the normalisation and explains how.

## Re-recording a `.stderr`

`just bless` updates the `.stderr` fixtures. Read the diff it leaves.

## Generated documentation site (`tools/gen-docs/`)

The lint catalogue at <https://ksxgithub.github.io/perfectionist/>
is rendered by `tools/gen-docs/` into a single, self-contained
`gh-pages/index.html`. These preferences shape what may go on
the page:

- **CSS over JavaScript.** Reach for CSS first; only add an inline
  `<script>` when CSS genuinely can't express the behaviour. When
  a script is necessary, render the JS-controlled element with the
  HTML `hidden` attribute and have the script clear it once its
  handlers are wired up. That covers every "JS isn't running"
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
  for older Firefox / Safari / mobile-Chromium engines, rather
  than only `@supports`-gated paths, since those still leave older
  browsers feature-less.

### Verifying rendering changes

`just all` doesn't cover layout, so a visual change can slip in a
regression that no automated step catches. Humans verify by opening
`gh-pages/index.html` in a browser. Agents that can't do that should
drive a headless Chromium from a throwaway Playwright script:

- Use Playwright's Chromium headlessly. If the environment already
  has Playwright and its Chromium binary available, point the
  launcher at them; otherwise install Playwright into a temporary
  location and use that. The standard
  `npm i -D playwright && npx playwright install chromium` works.
  Write a one-off `.mjs` that imports `chromium` from
  `"playwright"`, launches it, loads `gh-pages/index.html` at the
  viewport sizes the change cares about, and dumps screenshots
  and/or `getBoundingClientRect` / `getComputedStyle` values.

- Treat the script as scratch, and keep it out of the repository
  entirely. Write temporary files to a directory outside the
  project directory (such as `/tmp`), or to a directory named
  `tmp/`, or under a name matching the `tmp.*` pattern.

- Chromium only. For cross-engine concerns (Firefox / Safari
  support of a CSS feature), pair the screenshots with a caniuse
  check, since Chromium support is not proof a feature is safe to
  ship.

## Rationale section: "Why is this bad?" vs "Why restrict this?"

Every rule's documentation needs a section that explains the
motivation. The rustdoc on the `declare_tool_lint!` block needs one,
and so does the planning file in `planned-rules/`.
Pick the heading based on whether the violation is objectively
broken or a stylistic preference:

- **"Why is this bad?"**: use only when violating the rule
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
- **"Why restrict this?"**: use for every other rule. Most lints
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
start. It re-parses the crate's module source files from a
shared `SourceMap` so a rule reaching separate-file submodules
still sees `#[cfg(...)]` gates intact. It exists because this
exact bug has been written twice: a source-layout rule shipped as
a pre-expansion `EarlyLintPass` silently skips every separate-file
submodule. Before implementing any rule that reads
the *written layout* of items across module scopes, read
[Reaching every module (source-layout rules)](planned-rules/IMPLEMENTATION_CONVENTIONS.md#reaching-every-module-source-layout-rules).
The planning files document who depends on whom in their
"Interaction with sibling rules" sections. When implementing the
*first* rule in a dependency cluster, factor the shared helper
into a crate-internal module so the second rule can reuse it. The
planning files name this expectation explicitly; don't duplicate
the helper.

Another pair answers "is this test code?": `crate::test_code` and
`crate::cargo_target`. A rule that exempts test code uses these
rather than matching `cfg(test)` itself; before adding such an
exemption, read
[Recognising test-exclusive code](planned-rules/IMPLEMENTATION_CONVENTIONS.md#recognising-test-exclusive-code).

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
style: a `type(scope): subject` header with the type drawn from
the usual set (`feat`, `fix`, `docs`, `chore`, `ci`, `refactor`,
`style`, `build`, `test`, `perf`), an optional scope in
parentheses, and a `!` after the type/scope for breaking changes
(e.g. `feat!: remove ...`). New commits you author should match.

The single exception is version-bump commits, whose subject is
just the version itself (e.g. `0.0.0-rc.6`). Use this form only
for commits that do nothing other than bump the version.

The subject is **one line**. The [`commit-msg`
hook](.githooks/commit-msg) rejects any commit whose first paragraph
spans more than one line, with no opt-out.

Keep the subject at **72 characters or fewer**, the conventional
hard cap; 50 is the ideal. GitHub's web UI truncates a longer
subject, appends a `…`, and folds the overflow into the body behind
another `…`, so the commit reads as a mangled title/body split. The
[`commit-msg` hook](.githooks/commit-msg) rejects any commit whose
subject exceeds the cap (version-bump commits are exempt, but are
short anyway); it measures characters, not bytes, so an accented or
non-Latin subject is judged by what GitHub displays. Move any detail
that does not fit into the commit body: a blank line after the
subject, then the rest.

When a longer subject is genuinely warranted, set
`PERFECTIONIST_GIT_HOOK_ALLOW_LONG_SUBJECT=true` for that `git
commit` to skip just this check; the hook's other checks still
apply. `false`, empty, and unset all enforce the cap, and any other
value is rejected so a misspelt opt-out never silently fails to
apply.

## Issue and PR references in commit messages

Never write a **bare** `#NNN` reference in a commit message. A
reference is *bare* when a `#` followed by digits is not
immediately preceded by an ASCII word char (`[0-9A-Za-z_]`). That
is the form GitHub autolinks against *this* repo, so a number
meant for another project links to the wrong issue. Thus `#123`, `(#123)`,
`path/#123` are bare; `owner/repo#123`, `C#123`, `foo#123` are
not. The [`commit-msg` hook](.githooks/commit-msg) rejects any
commit containing one; install it on a fresh checkout with
`just install-git-hooks` (or `git config core.hooksPath .githooks`).

Use an unambiguous form instead:

- **Same repo:** `KSXGitHub/perfectionist#123`
- **Another repo:** `owner/repo#123`
- **Absolute URL:** `https://github.com/owner/repo/issues/123`

To enumerate list items, number them without `#` ("item 1", "1.",
"(1)"), since "`#1`"/"`#2`" otherwise autolink as issue 1/2. The hook
skips comment lines and the `git commit -v` diff, and ignores
non-bare forms like `#123abc` / `#L123` / `#issuecomment-1`.

## `@handle` mentions in commit messages

Never write a **bare** `@handle` in a commit message. Always wrap
it in backticks (`` `@foo` ``). Outside backticks GitHub autolinks `@foo`
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

Every other markdown file in the repository is only rendered in
contexts where relative paths resolve correctly (GitHub, local
editors, agent tooling). That covers `CLAUDE.md`,
`planned-rules/*.md` and anything else committed alongside the
source. Prefer relative links in those files; they survive
repository renames and don't bake in a hosting URL.

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

The constraint is not "GitHub invented it" but "the other
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

So the ones to keep out of rustdoc-bound docs are **alerts, task
lists, and mermaid fences**. The alert is the trap worth naming:
its marker is left as literal text in both the in-tree catalogue
and the docs site, where it reads as a typo.

`<details>` / `<summary>` renders everywhere, but still does
not belong in rustdoc-bound docs. `rules/*.md` exists so the
catalogue can be read in an editor without a browser, and
`tools/gen-docs/src/render_md.rs` drops the HTML renderer's
`<details>` panels for that reason; raw HTML in the markdown copy
works against its purpose.

### Choosing the alert type

GitHub defines these, and their plain meanings apply: `[!NOTE]`,
`[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]`. The one
editorial rule worth stating is that `[!WARNING]` and
`[!CAUTION]` mark an actual hazard, such as data loss, breakage or
a security issue, never emphasis. A spec caveat is not a hazard,
and reserving those two is what keeps them worth reading.

### A blockquote is not always an alert

`>` is also plain markdown for a quotation, and the catalogue
relies on that. Do not run a sweep that mechanically upgrades
every `>` block to an alert; read each one and leave it as it
stands. These kinds are worth naming:

- **An aside**: a remark in the author's own voice, addressed to
  the reader. This is the one an alert improves; pick its type by
  the rules above.
- **A sketch of emitted diagnostic text**, typically introduced
  by a line ending "should emit text along these lines:". Leave
  it as a blockquote: the markup inside is part of the sketch, so
  a `text` fence would render the `**error:**` markers literally
  and lose what the block is showing.

## Symlinks

This file is the authoritative implementation guide. It is also
exposed under other names so that other AI assistants and agent
harnesses pick it up automatically:

- `AGENTS.md` (symlink to `CLAUDE.md`).
- `.github/copilot-instructions.md` (symlink to `CLAUDE.md`).

Edit `CLAUDE.md` only; the symlinks pick up the change.
