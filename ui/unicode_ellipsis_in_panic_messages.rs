// Macro invocations: each of the following should fire once.

fn _panics() {
    panic!("could not parse manifest…");
    unimplemented!("not yet implemented…");
    todo!("write this later…");
    unreachable!("can't happen…");
}

fn _asserts(left: i32, right: i32, flag: bool) {
    assert!(flag, "flag was false…");
    assert_eq!(left, right, "tree did not flatten…");
    assert_ne!(left, right, "values matched unexpectedly…");
    debug_assert!(flag, "flag was false…");
    debug_assert_eq!(left, right, "tree did not flatten…");
    debug_assert_ne!(left, right, "values matched unexpectedly…");
}

fn _expect() {
    let _: i32 = Some(1).expect("config missing...");
    let _: i32 = Some(1).expect("config missing-bad-1…");
    let _: i32 = Result::<i32, ()>::Ok(1).expect("config missing-bad-2…");
    let _: () = Result::<i32, ()>::Err(()).expect_err("expected error…");
}

// Should NOT fire: non-panic / non-expect contexts.
fn _quiet() {
    let _ = "string literal with ellipsis (not a panic)";
    eprintln!("log line with ellipsis");
    let _ = format!("formatted message with ellipsis");
    // The synthetic message inserted by bare `assert!(cond)` has no
    // U+2026 so it doesn't fire here either.
    assert!(true);
    // Literal expressed via escape — source has `\u{2026}`, not the
    // raw codepoint — is not flagged.
    panic!("escaped \u{2026} stays");
}

// Should NOT fire: ASCII `...` is already correct.
fn _ok() {
    panic!("could not parse manifest...");
    assert_eq!(1, 1, "values matched...");
    let _ = Some(1).expect("config missing...");
}

fn main() {}
