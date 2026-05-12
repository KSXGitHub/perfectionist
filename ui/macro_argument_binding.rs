macro_rules! my_macro {
    ($($item:expr),* $(,)?) => {{ $(let _ = $item;)* 0 }};
}

// `debug_assert_eq!` is on the built-in deny list. The first argument
// is a non-trivial method call; in release builds the macro folds to
// `if false { ... }` and the call never runs, leaving the map in a
// state the author did not intend. The literal `None` and the string
// literal panic message are trivial and accepted.
fn _motivating_bug(map: &mut Map) {
    debug_assert_eq!(map.insert(0, 0), None, "duplicate key");
}

// Bare `debug_assert!` likewise: the condition is a method call, the
// argument is non-trivial, the rule flags it.
fn _debug_assert_method_call() {
    debug_assert!(value().is_some());
}

// `debug_assert_ne!` flagged on both arguments. `value()` is a
// function call; `Some(0)` is a tuple-variant *call*, not a bare
// path. Only the bare-path form (e.g., `Some` as a function pointer)
// is trivial, so the constructor invocation is flagged separately.
fn _debug_assert_ne_call() {
    debug_assert_ne!(value(), Some(0));
}

// An uncatalogued macro under the default `AllowAndDeny` mode is
// neither denied nor allowed: the rule defaults to flagging non-
// trivial arguments. `value()` is non-trivial; `count` (a path) is
// trivial and accepted.
fn _unknown_macro_default_flags(count: u32) {
    let _ = my_macro!(value(), count);
}

// Allow-listed `format!` evaluates each argument exactly once. Even a
// non-trivial expression in a format-args slot is accepted.
fn _format_allow_listed() {
    let mut count: u32 = 0;
    let _ = format!("retrying {} times", {
        count += 1;
        count
    });
}

// Allow-listed `vec!`. Comma-form is the array-like shape the rule
// targets, but `vec!` is on the allow list, so non-trivial elements
// are accepted under the default config.
fn _vec_allow_listed() {
    let _ = vec![value(), value(), value()];
}

// Repeat-form `vec![v; count]` uses a top-level `;`, which signals
// that the invocation is not a comma-separated argument list. The
// rule skips the whole call.
fn _vec_repeat_form_skipped() {
    let _ = vec![value(); 4];
}

// Curly-brace invocation is out of scope: by convention the body is
// the macro's DSL, not a comma-separated argument list. Skipped even
// for a deny-listed macro name. `debug_assert!` accepts the brace
// form via the surrounding `macro_rules!` dispatch.
fn _brace_delimiter_skipped() {
    debug_assert! { value().is_some() }
}

// All seven trivial argument shapes — accepted under every mode. The
// outer `debug_assert_eq!` is on the deny list, so any non-trivial
// argument here would otherwise be flagged.
fn _all_trivial_shapes_accepted() {
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
    debug_assert_eq!(buffer[0], buffer[INDEX], "indexing trivial bases");
    debug_assert_eq!(*pointer, *pointer, "deref of a path");
    debug_assert_eq!(0u32 as u64, MAX as u64, "trivial cast");
    debug_assert_eq!(::std::u32::MAX, std::u32::MAX, "rooted path");
}

// Single-argument deny-listed call with a non-trivial expression.
fn _single_argument_deny() {
    debug_assert!(value().is_some());
}

// Empty top-level argument list — nothing to check, no false positive.
fn _empty_argument_list() {
    let _: Vec<u32> = vec![];
}

struct Map;
impl Map {
    fn insert(&mut self, _: u32, _: u32) -> Option<u32> {
        None
    }
}

const MAX: u32 = 0;
const INDEX: usize = 0;

fn value() -> Option<u32> {
    None
}

fn main() {}
