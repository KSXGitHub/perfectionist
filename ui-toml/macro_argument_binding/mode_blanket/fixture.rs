// `mode = "blanket"` flags every function-like or array-like
// invocation that carries a non-trivial argument, regardless of what
// list (if any) the macro would otherwise hit. `format!` is on the
// default allow list but blanket mode disregards the built-in allow
// list, so the non-trivial format argument gets flagged. `vec!` of
// trivial elements is still accepted because the elements themselves
// are trivial.

fn main() {
    let count: u32 = 0;
    let _ = format!("count = {}", value());
    let _ = vec![count, count, count];
}

fn value() -> u32 {
    0
}
