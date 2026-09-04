// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
macro_rules! my_macro {
    ($($item:expr),* $(,)?) => {{ $(let _ = $item;)* 0 }};
}

macro_rules! arrow_macro {
    ($name:ident => $value:expr) => {{
        let _ = $value;
        0
    }};
}

macro_rules! await_macro {
    ($($tokens:tt)*) => {{ 0 }};
}

macro_rules! assignment_macro {
    ($($tokens:tt)*) => {{ 0 }};
}

macro_rules! op_separator_macro {
    ($($tokens:tt)*) => {{ 0 }};
}

macro_rules! arrow_separator_macro {
    ($($tokens:tt)*) => {{ 0 }};
}

macro_rules! in_separator_macro {
    ($($tokens:tt)*) => {{ 0 }};
}

// Stand-ins for the `insta` snapshot-assertion family, which the
// rule recognises by tail-segment match. Each one swallows its
// arguments unevaluated; the fixture only needs the call site to
// parse, not to run.
macro_rules! assert_snapshot {
    ($($tokens:tt)*) => {{}};
}
macro_rules! assert_debug_snapshot {
    ($($tokens:tt)*) => {{}};
}
macro_rules! assert_yaml_snapshot {
    ($($tokens:tt)*) => {{}};
}

macro_rules! dsl_macro {
    ($($tokens:tt)*) => {{ 0 }};
}

macro_rules! json {
    ($($tokens:tt)*) => {{ 0 }};
}

macro_rules! hashmap {
    ($($tokens:tt)*) => {{ 0 }};
}

macro_rules! bail {
    ($($tokens:tt)*) => {{ 0 }};
}

macro_rules! ensure {
    ($($tokens:tt)*) => {{ 0 }};
}

// `debug_assert_eq!` is in the built-in deny set. The first argument
// is an impure method call; in release builds the macro folds to
// `if false { ... }` and the call never runs, leaving the map in a
// state the author did not intend. The literal `None` and the string
// literal panic message are pure and accepted.
fn _motivating_bug(map: &mut Map) {
    debug_assert_eq!(map.insert(0, 0), None, "duplicate key");
}

// Bare `debug_assert!` likewise: the condition is a method call, the
// argument is impure, the rule flags it.
fn _debug_assert_method_call() {
    debug_assert!(value().is_some());
}

// `debug_assert_ne!` flagged on both arguments. `value()` is a
// function call; `Some(0)` is a tuple-variant *call*, not a bare
// path. Only the bare-path form (e.g., `Some` as a function pointer)
// is pure, so the constructor invocation is flagged separately.
fn _debug_assert_ne_call() {
    debug_assert_ne!(value(), Some(0));
}

// An uncatalogued macro under the default `AllowAndDeny` mode is
// neither denied nor allowed: the rule defaults to flagging non-
// pure arguments. `value()` is impure; `count` (a path) is
// pure and accepted.
fn _unknown_macro_default_flags(count: u32) {
    let _ = my_macro!(value(), count);
}

// Allowed `format!` evaluates each argument exactly once. Even a
// impure expression in a format-args slot is accepted.
fn _format_in_allow_set() {
    let mut count: u32 = 0;
    let _ = format!("retrying {} times", {
        count += 1;
        count
    });
}

// Allowed `vec!`. Comma-form is the array-like shape the rule
// targets, but `vec!` is in the allow set, so impure elements
// are accepted under the default config.
fn _vec_in_allow_set() {
    let _ = vec![value(), value(), value()];
}

// Allowed `insta` snapshot-assertion macros. Each variant
// evaluates its value argument exactly once before serialising, so
// the rule accepts impure arguments under the default config.
// Tail-segment matching means the rule recognises both bare and
// path-qualified call sites.
fn _insta_snapshots_in_allow_set() {
    assert_snapshot!(value().unwrap());
    assert_debug_snapshot!(value().unwrap());
    assert_yaml_snapshot!(value().unwrap());
}

// Repeat-form `vec![v; count]` uses a top-level `;`, which signals
// that the invocation is not a comma-separated argument list. The
// rule skips the whole call.
fn _vec_repeat_form_skipped() {
    let _ = vec![value(); 4];
}

// Curly-brace invocation is out of scope: by convention the body is
// the macro's DSL, not a comma-separated argument list. Skipped even
// for a denied macro name. `debug_assert!` accepts the brace
// form via the surrounding `macro_rules!` dispatch.
fn _brace_delimiter_skipped() {
    debug_assert! { value().is_some() }
}

// A brace-delimited *argument* (the macro's outer delimiter is `(`,
// the argument's outer delimiter is `{`) carries DSL body syntax
// that isn't a Rust expression — `{"key": "value"}` (JSON-shaped),
// `{"key" => "value"}` (maplit-shaped). The rule cannot propose a
// let-bind rewrite for these because they don't compile in
// expression position, so the argument is treated as
// not-an-expression and skipped. The DSL signal is a top-level `:`
// (not in a `let` statement) or a top-level `=>`.
fn _brace_argument_with_dsl_markers_skipped() {
    let _ = dsl_macro!({ "key": "value" });
    let _ = dsl_macro!({ "key" => "value", "other" => "value" });
    // Real Rust block expressions still go through the regular
    // pure-expression analysis: `{ let x: T = e; x }` reaches the
    // walker (the `let` keyword whitelists the `:`) and bottoms
    // out as impure (block expressions aren't a pure atom),
    // matching the spec's "blocks are impure" classification.
}

// A `let`-binding inside a brace-argument may be preceded by
// outer attributes (`#[cfg(...)]`, `#[allow(...)]`) or doc
// comments; the `let`-whitelist must skip past them before
// checking the statement's first token. Without the skip, the
// `:` in `let x: T` after `#[allow(...)]` would be misread as
// a DSL key-position marker and the brace argument would be
// dropped from analysis entirely (false negative on attributed
// blocks).
//
// The observable consequence of the fix is *uniform* treatment
// of brace-argument blocks: with the leading attribute correctly
// skipped, the rule now classifies this block the same way it
// classifies the un-attributed `{ let x = e; x }` — a real Rust
// block, walked through `looks_like_expression`, then flagged
// by the pure-expression walker (blocks are not a pure atom
// shape). Pre-fix, the attribute caused a silent skip; post-fix,
// the block is flagged, matching the rule's "blocks are impure"
// policy.
fn _brace_argument_with_attributed_let_flagged() {
    let _ = dsl_macro!({
        #[allow(unused, reason = "ui fixture")]
        let x: u32 = MAX;
        x
    });
}

// The atom-shaped pure arguments — literal, path, reference,
// mut-reference, tuple-index, deref, indexed pure base, pure
// cast, rooted path — accepted under every mode. The outer
// `debug_assert_eq!` is in the deny set, so any impure argument
// here would otherwise be flagged. The grammar is open-ended
// (array literals / repeats are exercised in a dedicated fixture
// below; binary chains and pure-getter postfixes likewise); this
// function only covers the atom shapes.
fn _all_pure_shapes_accepted() {
    let pair: (u32, u32) = (0, 0);
    let buffer: [u32; 4] = [0; 4];
    let pointer: &u32 = &MAX;
    let owned: u32 = 0;
    let mut left: u32 = 0;
    let mut right: u32 = 0;
    debug_assert_eq!(0u32, MAX, "literal vs path");
    debug_assert_eq!(true, false, "bool keywords");
    debug_assert_eq!(&owned, &MAX, "references");
    debug_assert_ne!(&mut left, &mut right, "mut reference");
    debug_assert_eq!(pair.0, pair.1, "tuple index");
    debug_assert_eq!(buffer[0], buffer[INDEX], "indexing pure bases");
    debug_assert_eq!(*pointer, *pointer, "deref of a path");
    debug_assert_eq!(0u32 as u64, MAX as u64, "pure cast");
    debug_assert_eq!(::std::u32::MAX, std::u32::MAX, "rooted path");
    // The cast *target* is a rooted path too, which the type-position
    // parser reaches through a separate arm from the expression one
    // exercised by `rooted path` above.
    debug_assert_eq!(0u32 as ::std::primitive::u64, 0, "rooted cast target");
    let flag: bool = true;
    let mask: u32 = 0;
    debug_assert!(!flag, "unary not on a path");
    debug_assert_eq!(!mask, !MAX, "unary not on paths");
    debug_assert_eq!(!buffer[0], !buffer[INDEX], "unary not on pure suffix");
}

// Single-argument denied call with an impure expression.
fn _single_argument_deny() {
    debug_assert!(value().is_some());
}

// `.await` is `ExprKind::Await`, not a field access — impure
// per the spec. Without the explicit `await`-keyword rejection in
// the dot-suffix branch, the walker would consume `.await` as a
// "pure field access" and silently accept the whole expression.
// `await_macro!` is uncatalogued so default-mode flags impure
// args — that's what we verify here. The macro swallows the tokens
// rather than emitting an `expr` so the fixture stays valid under
// the test harness's default edition (no real `async fn` needed).
fn _await_suffix_flagged() {
    let future = ();
    let _ = await_macro!(future.await);
}

// Empty top-level argument list — nothing to check, no false positive.
fn _empty_argument_list() {
    let _: Vec<u32> = vec![];
}

// Skipped: a top-level `=>` signals the argument is a syntactic
// position the macro author chose (`Type => [LINT_NAMES]` is the
// canonical example, courtesy of `impl_lint_pass!`), not a Rust
// expression. The rule does not flag these — `value()` here is
// impure but lives on the right-hand side of `=>`, so the whole
// argument is skipped rather than parsed as an expression.
fn _fat_arrow_skips_argument() {
    let _ = arrow_macro!(NameType => value());
}

// Skipped: a top-level `=` or compound-assignment operator signals an
// assignment-shaped DSL matcher the macro author chose
// (`make_const!(NAME = 'x')`, `bump!(counter += 1)`), not a Rust
// expression to bind. `name = value` is technically a valid Rust
// assignment expression of unit type, but in macro-argument position
// it is overwhelmingly a structural separator and the let-bind
// rewrite would be meaningless for the macro's matcher arm.
fn _assignment_dsl_skipped(items: &mut u32, slot: &mut u32) {
    let _ = assignment_macro!(LEVEL0 = value());
    let _ = assignment_macro!(*items += value());
    let _ = assignment_macro!(*slot *= value());
}

// Skipped: a bare operator token cannot begin a Rust expression, so
// `==` here is not an expression at all; suggesting a `let` binding
// is impossible. Custom operator-positional DSLs like
// `debug_assert_op_expr!(actual, ==, expected)` rely on this skip.
fn _bare_operator_skipped(left: u32, right: u32) {
    let _ = op_separator_macro!(left, ==, right);
    let _ = op_separator_macro!(left, >, right);
}

// Skipped: a top-level `->` is a structural matcher token, not a Rust
// operator. Macros like `link!("src" -> "dst")` use it to pair two
// arguments; reparse-as-expression would fail and the let-bind hint is
// nonsense. Same skip applies to definition-shape DSLs that mix `->`
// with other separators (`test_case!(name -> value in System == "x")`).
fn _arrow_separator_skipped(left: u32, right: u32) {
    let _ = arrow_separator_macro!("src" -> "dst");
    let _ = arrow_separator_macro!(left -> right);
}

// Skipped: a top-level `in` keyword usually indicates a DSL separator
// (`for_each!(x in iter, ...)`-style matchers) rather than a Rust
// expression. A bare `for x in iter { ... }` macro argument is also a
// real Rust expression containing a top-level `in`, and the heuristic
// will skip that too; the trade-off favours the DSL-matcher case,
// which has been reported in the wild, over the `for`-expression case,
// which has not. The principled fix (#64) is a parser-based reparse.
fn _in_keyword_separator_skipped() {
    let _ = in_separator_macro!(item in container);
}

// Skipped: a definition-shape DSL that mixes `->`, `in`, and `==` as
// separator-position tokens. None of these positions form a single
// Rust expression standalone, and any `let` binding the rule could
// suggest would break the matcher's structural pattern. The `->`
// alone is enough to disqualify the argument; `==` stays a real Rust
// binary operator so that `debug_assert!(a == b)` keeps being pure.
fn _mixed_definition_dsl_skipped() {
    let _ = arrow_separator_macro!(plain_number -> 65_535 in PlainNumber == "65535");
}

// `()` is the canonical pure value: the empty-tuple / unit literal.
// Parenthesised pure expressions and tuples of pure elements are
// also pure, since none of these introduce a side effect beyond
// their contents. Without these, callers that pass `()` as a marker
// argument would be flagged with a meaningless let-binding hint.
fn _parenthesised_pure_accepted(point: (u32, u32)) {
    let _ = my_macro!((), value());
    let _ = my_macro!((MAX), value());
    let _ = my_macro!((point.0, point.1), value());
    let _ = my_macro!((MAX,), value());
}

// Array literals over pure elements are pure. The bracketed shape
// is the canonical Rust array literal `[a, b, c]` and the repeat
// form `[expr; count]`; both forms evaluate every captured Rust
// expression at most once and so don't introduce the side-effect
// hazard the rule is built to catch. Indexing (`base[index]`) is
// handled separately by the suffix walker and is unaffected.
fn _array_literal_of_pure_elements_accepted() {
    debug_assert_eq!([0, 1, 2], [MAX, MAX, MAX], "array literal");
    debug_assert_eq!([0; 4], [MAX; 4], "array repeat");
}

// Negative coverage: an array literal with at least one impure
// element is still flagged. The let-bind rewrite the rule suggests
// is meaningful here — the caller can bind the impure element
// first and pass the resulting array.
fn _array_literal_with_impure_element_flagged() {
    let _ = my_macro!([value(), value()]);
}

// Negative coverage on the array-repeat halves. `[expr; count]`
// is pure only when *both* halves are pure, so each side
// independently breaking purity must still flag. `await_macro!`
// is a `tt`-capturing stand-in that discards its body so the
// fixture compiles even though Rust would otherwise reject a
// non-const count in expression position; the rule's check runs
// on tokens, before that semantic constraint applies.
//
// `await_macro!` is uncatalogued, default-mode flags impure
// args, and the bracket atom routes the halves through
// `is_pure_expression` independently via `split_array_repeat`.
fn _array_repeat_with_impure_half_flagged() {
    let _ = await_macro!([value(); 4]);
    let _ = await_macro!([0; size()]);
}

// Binary chains over pure operands are pure — comparisons and
// arithmetic on local bindings, fields, and constants are side-effect-
// free and produce the same result regardless of how many times the
// macro evaluates them. Flagging these would defeat the debug-only
// optimisation of `debug_assert!(a <= b)` by forcing the comparison to
// evaluate in release builds.
fn _binary_chain_of_pure_operands_accepted(left: u32, right: u32, point: (u32, u32)) {
    debug_assert!(left <= right);
    debug_assert!(left == right);
    debug_assert!(left != right && left < MAX);
    debug_assert!(point.0 + point.1 < MAX);
    debug_assert!(left * 2 == right + 1);
}

// Pure-getter method calls on a pure base are pure postfixes.
// `len`, `is_empty`, `as_str`, `as_bytes`, `as_ref`, `as_mut`,
// `as_deref`, `as_slice` are the built-in pure-getter set; projects
// extend it via `dylint.toml`'s `extra_pure_methods` knob (see
// `tests/impure_macro_arguments.rs`). Combined with the binary-chain
// rule above, `debug_assert!(vec.len() <= cap)` no longer drags the
// comparison out of its `cfg(debug_assertions)` guard.
//
// `text: &String` (rather than `&str`) is deliberate: `str::as_str`
// is currently nightly-only behind `str_as_str`, so `&str.as_str()`
// would refuse to compile under the test harness's stable check.
// `String::as_str` is stable and lets the fixture exercise the
// pure-getter rule on a string-shaped receiver.
fn _pure_method_postfix_accepted(slice: &[u32], text: &String) {
    debug_assert!(slice.len() <= MAX as usize);
    debug_assert!(slice.is_empty() || slice.len() < MAX as usize);
    debug_assert!(text.as_bytes().len() == text.as_str().len());
    debug_assert!(slice.as_ref().len() == slice.len());
}

// Negative coverage: a zero-arg method whose name is *outside* the
// built-in pure-getter list still flags. `clear` is a state-mutating
// method despite its zero-arg shape, so the rule must keep flagging
// it under the default config (users who want it accepted explicitly
// opt in via `extra_pure_methods`).
fn _zero_arg_mutating_method_flagged(slice: &mut Vec<u32>) {
    debug_assert!(slice.clear() == ());
}

// Negative coverage: a turbofish-generic method call is impure.
// The `::<T>` token sequence sits between `.method` and `()`, so the
// suffix walker's `.method()` recogniser does not match and the
// argument falls through to the impure bucket. Matches the
// docstring promise that "method calls with arguments, generic
// method calls, ... still flag".
fn _turbofish_method_call_flagged(text: &str) {
    debug_assert!(text.parse::<u32>().is_ok());
}

// Third-party `tt`-based literal builders on the curated allow
// list (`serde_json::json!`, `maplit::{hashmap, btreemap, ...}!`,
// `anyhow::{bail, ensure}!`) accept impure top-level arguments
// without flagging. Each macro evaluates every captured Rust
// expression exactly once at runtime — no cfg gate, no repeated
// substitution — so the exactly-once contract the rule defends
// already holds. Tail-segment matching means `serde_json::json!`,
// `::serde_json::json!`, and the bare `json!` form all line up
// with the same entry.
//
// Each call below was chosen so the allow-set entry is *load-
// bearing*: the impure argument shape would otherwise be flagged
// by the default `AllowAndDeny` mode. Without that property the
// test passes trivially and a regression that drops the entry
// from `BUILTIN_ALLOW` would slip through.
//
// - `json!(value().unwrap())` — direct impure expression, no
//   DSL marker, not brace-delimited. Without `json` in the allow
//   set, the rule reaches `is_pure_expression` and flags it.
// - `bail!("error: {}", value())` — second argument is an impure
//   function call. Without `bail`, the second-argument check
//   flags it.
// - `ensure!(value().is_some(), "missing: {}", value())` — first
//   argument is an impure method call; second / third are
//   impure. Without `ensure`, all three flag.
// - `json!({...})` and `hashmap!(k => v)` are also exercised, but
//   note these would *also* be skipped by the brace-DSL and
//   `=>`-DSL heuristics respectively even without their
//   `BUILTIN_ALLOW` entries — they're here for shape coverage,
//   not as load-bearing assertions. The `maplit::*` entries are
//   in the same boat by design: every form of the macros emits
//   `=>` at top level, which the DSL-marker check already
//   skips. The allow-set entries serve as documentation of
//   intent more than functional gates.
fn _third_party_in_allow_set_accepted() {
    let _ = json!(value().unwrap());
    let _ = json!({ "ts": value(), "items": [value(), value()] });
    let _ = hashmap!("a" => value(), "b" => value());
    let _ = bail!("error: {}", value());
    let _ = ensure!(value().is_some(), "missing: {}", value());
}

// `core` / `std` compile-time macros (`concat!`, `env!`,
// `option_env!`, `include_str!`, `include_bytes!`, `stringify!`,
// `cfg!`, `line!`, `column!`, `file!`, `module_path!`) are on the
// allow set AND count as pure atoms when nested inside another
// macro. The expansion is a compile-time constant — a literal, a
// `&'static str`, a `bool`, a span marker — with no runtime
// evaluation to disturb. Both behaviours together make the patterns
// flagged in issue #71 invisible to the lint.
fn _compile_time_macros_accepted() {
    // Outer compile-time macros are in the allow set: their arguments
    // (literals, paths, other compile-time macro calls) are
    // unconditionally accepted.
    let _ = concat!("home is at ", env!("CARGO_PKG_NAME"));
    let _ = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));
    let _ = stringify!(let x = compute(););
    // Inner compile-time macros are pure atoms inside any
    // surrounding (even denied) macro: there is no runtime
    // expression to bind.
    debug_assert_eq!(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_NAME"));
    debug_assert!(cfg!(any()) || cfg!(all()));
    debug_assert_eq!(line!(), line!());
    debug_assert_eq!(concat!("a", "b"), "ab");
    // Qualified-path call sites still hit the pure-macro
    // recognition: tail-segment matching makes `::std::stringify!`
    // line up with the same `"stringify"` entry as the bare form.
    debug_assert_eq!(::std::stringify!(x), std::stringify!(x));
}

#[allow(dead_code, reason = "ui fixture")]
struct NameType;

struct Map;
impl Map {
    fn insert(&mut self, _: u32, _: u32) -> Option<u32> {
        None
    }
}

const MAX: u32 = 0;
const INDEX: usize = 0;

fn size() -> usize {
    0
}

fn value() -> Option<u32> {
    None
}

fn main() {}
