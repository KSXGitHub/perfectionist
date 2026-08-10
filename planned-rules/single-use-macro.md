# `single_use_macro`

**Source:** project convention. A declarative macro exists to
*reuse* a code shape. A private one that is expanded exactly once
delivers no reuse — the single expansion could just as well be
written where it is used. The rule is the `macro_rules!`
counterpart of `clippy::single_call_fn` (which flags a private
function called from exactly one place) and shares the family
resemblance of the rustc lints `single_use_lifetimes` (a lifetime
named once should be elided) and `unused_macros` (a `macro_rules!`
used *zero* times should be deleted). This rule owns the gap
between those last two: rustc's `unused_macros` fires at a use
count of `0`, this rule fires at exactly `1`, and `2+` is genuine
reuse the rule leaves alone.

## Statement

A **private** declarative macro (`macro_rules!`, or a macros-2.0
`macro` without `pub`) that is **instantiated exactly once** in
the crate is a macro that buys nothing over inlining its body at
the single call site. The rule flags the definition.

```rust
// Avoid: a private macro with exactly one call site.
macro_rules! check_bounds {
    ($idx:expr, $len:expr) => {
        assert!($idx < $len, "index {} out of bounds for len {}", $idx, $len);
    };
}

fn get(v: &[u8], idx: usize) -> u8 {
    check_bounds!(idx, v.len());
    v[idx]
}

// Prefer: inline the expansion (or, for an expression body, a
// function / `const`). No macro layer, so the reader, the type
// checker, and the IDE all see the real code directly.
fn get(v: &[u8], idx: usize) -> u8 {
    assert!(idx < v.len(), "index {idx} out of bounds for len {}", v.len());
    v[idx]
}
```

The value of the rule is in the boundary: it must fire on the
single-use macro above and stay silent on every macro that earns
its definition — public macros, macros expanded two or more times,
and macros whose one expansion still fans code out (see "When the
rule stays silent"). Those two halves — the trigger and its
complement — are both load-bearing.

## Why restrict this?

This is a stylistic preference, not a correctness issue. A macro
invoked once compiles and runs exactly as its inlined body would;
nothing is broken by keeping it. The project prefers to inline the
single-use case because a macro carries costs a function or an
inline block does not, and those costs are only worth paying when
reuse amortises them:

- **A macro is opaque until it expands.** Name resolution, type
  checking, and borrow checking all run on the *expansion*, so an
  error is reported against generated tokens and traced back to
  the call site by hand. A function is checked at its definition;
  an inline block is checked where it sits. For a shape used once,
  the macro's indirection buys nothing and taxes every reader.
- **Editor tooling degrades inside a macro.** Go-to-definition,
  rename, type-on-hover, and inline diagnostics work far better on
  a function or a plain block than on `macro_rules!` transcriber
  tokens. One call site is not enough reuse to justify surrendering
  them.
- **The reader pays a lookup with no payoff.** Encountering
  `check_bounds!(idx, v.len())` forces a jump to the definition to
  learn what it does — the same tax a helper function levies, but a
  function used once is itself a smell (`clippy::single_call_fn`),
  and a macro is the heavier form of it.
- **"Reuse" is the macro's whole justification.** A second call
  site changes the calculus: now the macro removes duplication, or
  expresses something a function cannot (a shape over syntax,
  captured control flow, item generation). At exactly one call site
  none of that reuse exists yet, so the definition is speculative.

The counterpart in the language's own lints is `single_use_lifetimes`
and `single_call_fn`: naming something once and then using it once is
the ceremony without the benefit.

## What makes a macro a single-use anti-pattern

All of the following must hold:

1. **It is a declarative macro defined in this crate.** A
   `macro_rules!` item, or a macros-2.0 `macro` item. Procedural
   macros (function-like, derive, attribute) are defined in a
   separate `proc-macro` crate and cannot be defined-and-used in
   the same crate, so they never reach this trigger — which is why
   the name says "macro" without qualification: within one crate a
   define-and-use macro is necessarily declarative.
2. **It is private.** Not reachable from outside the crate: no
   `#[macro_export]`, and not surfaced through any `pub use`
   re-export chain (nor declared `pub macro`). A macro that is part
   of the crate's public API is used by callers the lint cannot
   see, so a single *in-crate* use says nothing about its reuse.
3. **It is instantiated exactly once.** Exactly one macro
   expansion in the compiled crate resolves to this definition. Not
   zero (that is `unused_macros`' job), not two or more.
4. **Its single expansion does not fan code out.** By default, a
   macro whose transcriber contains a fragment repetition
   (`$(...)*`, `$(...)+`, `$(...)?`) is exempt even at one call
   site — see the next section for why one invocation of such a
   macro is still reuse.

## When the rule stays silent

The complement of the trigger. Each of these is a macro that pays
for itself despite (or because of) how it is counted, and the lint
must not fire on any of them.

- **Zero uses.** Owned by rustc's `unused_macros` (warn-by-default),
  whose remedy is *delete*, not *inline*. This rule deliberately
  starts at one so the two never both fire on the same definition.

- **Two or more instantiations.** Real reuse — the definition earns
  its keep — so the rule is silent. Counting *instantiations* (not
  textual call sites) makes this robust in two directions that a
  naïve source scan gets wrong:
  - **Recursion is reuse, and counts as reuse for free.** A
    recursive macro invoked once by the user re-instantiates itself
    while it recurses; each self-call is another expansion of the
    same definition, so the instantiation count is `≥ 2` and the
    rule stays silent. No special recursion case is needed — the
    count already reflects that the body does real, repeated work.
  - **A macro called inside another macro is counted per
    instantiation.** If `inner!` is written once inside `outer!`'s
    body and `outer!` is used three times, `inner!` is instantiated
    three times. The rule sees the reuse and stays silent, matching
    the intuition that `inner!` is factored-out shared code.

- **Fan-out over a repetition (default exempt).** A macro invoked
  once whose transcriber repeats a captured fragment reuses its body
  across the *elements of one call*, not across call sites. Inlining
  would mean hand-writing every repeated copy — exactly the
  duplication the macro removes.

  ```rust
  // Not flagged: one call site, but the body fans out over `$(...)*`.
  macro_rules! declare_status_codes {
      ($($name:ident = $code:literal),* $(,)?) => {
          $( pub const $name: u16 = $code; )*
      };
  }
  declare_status_codes! {
      OK = 200,
      NOT_FOUND = 404,
      TEAPOT = 418,
  }
  ```

  The repetition is the signal that the single invocation is doing
  N-way code generation. Firing here and telling the author to
  "inline it" would be wrong: there is nothing shorter to inline to.
  Controlled by `exempt_repetition_bodied` (default `true`).

- **Public / re-exported macros.** `#[macro_export]` publishes the
  macro at the crate root for downstream crates; a `pub use
  path::to::mac;` re-export makes an otherwise-private
  `macro_rules!` part of the public API. Either way the true use
  count includes callers outside this crate, so the in-crate count
  is not the whole story and the rule abstains.

- **Proc-macro-synthesised definitions.** A `macro_rules!` emitted
  by another macro's expansion is not something the author can
  inline. Suppressed by the usual guard (see Implementation notes).

## Configuration

```toml
# dylint.toml
["perfectionist::single_use_macro"]
# Exempt a single-use macro whose transcriber repeats a captured
# fragment (`$(...)*` / `$(...)+` / `$(...)?`). One invocation of
# such a macro still fans its body out across the elements of the
# call, which is reuse that inlining cannot express more concisely.
# Defaults to `true`.
exempt_repetition_bodied = true

# Also exempt a single-use macro that has two or more matcher arms.
# Multiple arms are a (weak) signal that the macro was written to
# dispatch over several input shapes — e.g. a recursive macro whose
# one call happens to hit only the base-case arm, so instantiation
# counting alone does not see the recursion. Off by default because
# a trailing-comma convenience arm (`($(,)?) => {}`) also trips it,
# and such a macro used once is still a genuine single-use macro.
# Defaults to `false`.
exempt_multi_arm = false
```

The two knobs are independent on/off switches, so they are two
boolean fields rather than one `exempt = [...]` array, per the
config-shape convention in
[`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md).

## What to lint

`LateLintPass`. The rule needs three facilities that only exist
after the crate is compiled far enough for a late pass: the macro's
`DefId`, its effective visibility, and the crate's macro-expansion
records. It is **not** a source-layout rule and does **not** route
through `src/module_reparse.rs`: it reads a *semantic* property
(how many times a definition was instantiated), and the late pass's
HIR plus the expansion table already span every module and
separate file, so the "reaching every module" trap
([`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md))
does not apply. The one thing the expansion table cannot see —
`#[cfg]`-disabled call sites — is discussed under Limitations.

1. **Collect candidate definitions.** Walk HIR items for
   `ItemKind::Macro(_, MacroKind::Bang)` (both `macro_rules!` and
   macros-2.0 `macro`; the `MacroDef.macro_rules` flag distinguishes
   them but both are in scope). For each, record its `LocalDefId`.
2. **Drop the public ones.** Skip any candidate that is externally
   reachable:
   `cx.tcx.effective_visibilities(()).is_public_at_level(def_id, Level::Reexported)`
   — the same reachability test
   `exhaustive_error_enums` (`src/rules/exhaustive_error_enums.rs`)
   uses. This subsumes both `#[macro_export]` (public at crate root)
   and `pub use` re-exports (public at the re-export) in one query,
   so the rule does not hand-roll a scan for either.
3. **Count instantiations per definition.** For every macro
   expansion in the crate, read its
   `ExpnData { macro_def_id, kind: ExpnKind::Macro(MacroKind::Bang, _), .. }`
   and tally one per distinct `ExpnId` whose `macro_def_id` is a
   candidate. Instantiations are discovered the way
   `impure_macro_arguments` reaches a call's definition — through
   `Span::ctxt().outer_expn_data().macro_def_id` — but here the pass
   walks the whole crate collecting the *set* of expansion ids
   rather than inspecting one call. In practice: visit every HIR
   node, walk its `SyntaxContext` outward
   (`ExpnData::call_site.ctxt()` chains one expansion to the next),
   and insert each `ExpnId` into a `FxHashMap<DefId, FxHashSet<ExpnId>>`
   keyed by `macro_def_id`. Deduplicating by `ExpnId` is what makes
   the count "instantiations", not "generated HIR nodes": one call
   emits many nodes under a single `ExpnId`.
4. **Apply the exemptions.** For each candidate with exactly one
   instantiation:
   - if `exempt_repetition_bodied` and any matcher arm's *transcriber*
     contains a `TokenTree` repetition, skip;
   - if `exempt_multi_arm` and the `macro_rules!` has two or more
     arms, skip.
5. **Emit** on the definition. The diagnostic names the macro and
   points a secondary span at the single call site (recovered from
   the surviving `ExpnId`'s `call_site`).

### Difficulty

**Hard.** Like `manual-lazy-init`, the trigger is a whole-crate
negative: proving a definition is used *exactly* once means visiting
every expansion in the crate, not matching one local pattern. The
count itself is cheap (one crate walk into a hash map), but the
correctness lives in the edges — recursion, macro-in-macro nesting,
cfg-stripped call sites, and expansions that leave no HIR node — each
of which pushes the count off by one in a way that flips the verdict.
The discovery and visibility steps reuse machinery already in the
tree; the instantiation census is the new part.

## Implementation notes

- **No autofix.** Inlining a macro correctly means re-running its
  transcriber on the invocation's tokens under the macro's own
  hygiene — `$crate`, metavariable hygiene, `local_inner_macros` —
  which the lint cannot reproduce faithfully from the outside; and
  the alternative "make it a function / `const`" rewrite depends on
  whether the body is an expression, a run of statements, or item
  definitions, and on what it captures from the call-site scope. Both
  are judgment calls, so the diagnostic is informational, matching
  `impure_macro_arguments`' let-binding hint (advice, no rewrite).

- **Proc-macro guard.** The diagnostic's primary span is the whole
  macro definition, which is wider than a bare identifier, so the
  built-in `report_in_external_macro: false` filter most likely
  already covers it — but a `macro_rules!` synthesised by an outer
  macro must not be flagged, so confirm with a
  `ui/single_use_macro_proc_macro.rs` fixture (a derive that emits a
  private, single-use, non-repetition `macro_rules!` the rule would
  otherwise fire on) and add `crate::common::hir_in_external_macro`
  if it fires. Mutation-check the fixture per the "Suppressing
  proc-macro-synthesised violations" section of
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md):
  delete the guard, confirm the fixture turns red, restore it.

### Limitations

Two ways the instantiation census can miscount, both worth stating
in code (not the user-facing `declare_tool_lint!` doc) so a later
reader knows they were considered:

- **`#[cfg]`-disabled call sites are invisible.** The expansion
  table only records macros that were actually expanded, i.e. call
  sites in the *enabled* cfg configuration. A macro invoked once
  under `#[cfg(unix)]` and once under `#[cfg(windows)]` is expanded
  once in any single build, so the census reads `1` and the rule
  would fire — yet the macro is reused across configurations and
  inlining it would duplicate the body into both branches. This is
  the same cfg-visibility gap that the source-layout rules solve
  with `module_reparse`, but here re-parsing does not help: the
  cross-cfg *identity* of a call still needs name resolution the
  pre-expansion AST lacks. The conservative mitigation is to skip a
  candidate whose sole instantiation's `call_site` is lexically
  under a `#[cfg]` gate (or whose definition is), trading a few
  missed real single-use macros for not firing on the cfg-split
  case; a project that hits a false positive here can also
  `#[allow]` the one site. Ship the simple census first and layer
  the cfg guard on if it proves necessary.
- **Expansions that leave no HIR node.** A macro that expands to
  nothing, or whose entire output is consumed as tokens by an
  enclosing macro, contributes no node for the span walk to find, so
  a second such use could go uncounted and drop the census to a
  false `1`. This is rare; the honest conservative stance is that the
  census counts *observed* instantiations and a definition with no
  observed instantiation at all is left to `unused_macros`.

## Default state

Active by default. The anti-pattern is common — especially in
machine-generated code, which reaches for a macro to stamp out a
shape and then stamps it once — and the default exemptions
(repetition fan-out, public macros, recursion via the instantiation
count) remove the classes where a single use is legitimate. It is
`Active` rather than `Inactive` even though `clippy::single_call_fn`
(its function-shaped cousin) is allow-by-default, because a macro is
the costlier abstraction of the two: it defeats type-checking and
tooling in ways a function does not, so a single-use macro is a
stronger smell than a single-use function. A project that leans on
single-use macros deliberately (or lives with the cfg-split caveat
above) turns the rule off via `[perfectionist].disable`.

## Interaction with clippy and sibling rules

- **`clippy::single_call_fn`** is the same anti-pattern one
  construct over — a private *function* called from exactly one
  place. This rule is its `macro_rules!` analogue; the two do not
  overlap (a function is not a macro) and a project may reasonably
  run both. The name is not *mirrored* from clippy (that convention
  is reserved for a genuine refinement of a like-named clippy lint,
  per
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md));
  `single_use_macro` is a fresh anti-pattern name in clippy's
  `single_*` idiom, chosen so `#[allow(perfectionist::single_use_macro)]`
  reads as "permit this single-use macro".
- **rustc `unused_macros`** owns the adjacent zero-use case (remedy:
  delete). This rule starts at one (remedy: inline) so the two are
  disjoint and never double-report.
- **rustc `single_use_lifetimes`** is the closest existing lint in
  spirit — "named once, used once, so elide the name" — and the
  source of the `single_use_*` naming shape.
- [`impure-macro-arguments.md`](./impure-macro-arguments.md) and
  [`macro-trailing-comma.md`](./macro-trailing-comma.md) inspect
  macro *invocations*; this rule inspects macro *definitions* and
  their crate-wide use count, so their triggers do not overlap. It
  does reuse the same `outer_expn_data().macro_def_id` bridge from a
  call back to its definition that `impure_macro_arguments` relies on
  for its late-pass resolution.
- See
  [`IMPLEMENTATION_CONVENTIONS.md`](./IMPLEMENTATION_CONVENTIONS.md)
  for the cross-cutting conventions every rule follows, in
  particular the `perfectionist::*` lint-name namespacing.
</content>
</invoke>
