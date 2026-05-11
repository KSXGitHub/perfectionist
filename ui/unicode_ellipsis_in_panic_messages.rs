// Macro invocations: each of the following should fire once
// per U+2026 codepoint in the literal.

fn _panics() {
    panic!("could not parse manifest…");
    unimplemented!("not yet implemented…");
    todo!("write this later…");
    unreachable!("can't happen…");
    // Raw string literals are scanned identically.
    panic!(r"raw string with …");
    panic!(r#"raw-hash string with …"#);
    // Two ellipses in one literal — should fire twice.
    panic!("first … and second …");
    // Path-qualified macro invocation still resolves to `panic`.
    std::panic!("qualified path with …");
    // Bracket and brace delimiters are recognised by the depth
    // tracker.
    panic!["bracket form with …"];
    panic! {"brace form with …"}
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

// Should NOT fire: non-panic / non-expect contexts. Each line below
// contains a real U+2026, so a regression that started flagging these
// contexts would surface here.
fn _quiet() {
    let _ = "string literal with …, not a panic";
    eprintln!("log line with …");
    let _ = format!("formatted message with …");
    // The synthetic message inserted by bare `assert!(cond)` has no
    // U+2026, so it doesn't fire here either.
    assert!(true);
    // Literal expressed via escape — source has `\u{2026}`, not the
    // raw codepoint — is not flagged.
    panic!("escaped \u{2026} stays");
    // String literals nested inside another macro that is *not* a
    // panic-family member are not part of the panic message and must
    // not be flagged, even when they appear inside a panic-family
    // call. The outer message ("path:") is ASCII so the lint stays
    // silent on this whole line.
    panic!("path: {}", concat!("dir/", "name…"));
    // Value-position literals in `assert_eq!`/`assert_ne!` and their
    // `debug_*` variants are the values being compared, not the
    // panic message. Flagging (and especially autofixing) them would
    // change what the assertion tests for.
    assert_eq!("value-with-…", "another-…");
    assert_ne!("value-with-…", "another-…");
    debug_assert_eq!("value-with-…", "another-…");
    debug_assert_ne!("value-with-…", "another-…");
    // `assert!`'s first argument is the condition — a string-literal
    // condition wouldn't compile, so this case is more theoretical,
    // but the skip-count keeps it consistent. The trailing literal
    // *is* the message and should still fire.
    assert!(true, "message with …");
}

// Should NOT fire: ASCII `...` is already correct.
fn _ok() {
    panic!("could not parse manifest...");
    assert_eq!(1, 1, "values matched...");
    let _ = Some(1).expect("config missing...");
}

fn main() {}
