//! End-to-end proof that the `redundant_derive_more_forward_template`
//! autofix is a no-op, against the real `derive_more`.
//!
//! ## What this actually tests
//!
//! Not `derive_more`'s behaviour — that would be a tautology. The
//! subject is *this crate's trigger*, and `derive_more` is only the
//! oracle it is judged against. Each case is compiled, then run through
//! `cargo dylint --fix`, so the rule itself decides what gets deleted
//! and its own suggestion span performs the deletion. Whatever the fixer
//! touched must then expand to byte-identical generated code — same
//! body, same `where` clause — and still compile. A trigger that grows
//! to cover a new shape is therefore checked automatically; nothing here
//! restates a hand-maintained list of what the rule is believed to do.
//!
//! The converse matters too, so a case the fixer leaves alone is
//! compared against the same attribute deleted by hand: if that changes
//! nothing, the rule is refusing a shape it could have fixed, and the
//! bail-out has to justify itself or go.
//!
//! Reading the expander is not a substitute. `additional_deref_args`
//! and `generate_bounds` each look decisive in isolation and are not,
//! and reasoning about them produced a wrong answer in both directions
//! before this test existed.
//!
//! Like the other integration tests here, the fixture is materialised
//! fresh in a `TempDir` while `CARGO_TARGET_DIR` points at the warmed
//! `target/integration-fixtures`, so the compiled std and the built
//! perfectionist plugin are reused instead of paid for from cold. Only
//! `derive_more` is new to that cache, and only on the first run.
//!
//! Ignored by default: it fetches `derive_more`, runs the fixer and
//! three expansions, which does not belong in the gating suite.
//!
//! ```text
//! cargo test --test autofix_no_op -- --ignored --nocapture
//! ```

pub mod _utils;

use _utils::{TempDir, cargo_manifest_dir, fixture_dylint_toml, shared_target_dir};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

/// `(name, as the user writes it, with the attribute deleted by hand)`.
///
/// The second form is *not* the expected fix — the fixer produces that
/// itself. It is only used to ask whether a shape the rule declined was
/// one it could safely have fixed.
type Case = (&'static str, &'static str, &'static str);

/// Cases are one line each so the fixer's line-deletion leaves a
/// well-formed module either way.
const CASES: &[Case] = &[
    (
        "tuple_newtype",
        r#"#[derive(derive_more::Display)] #[display("{_0}")] pub struct S(pub String);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub String);"#,
    ),
    (
        "named_field",
        r#"#[derive(derive_more::Display)] #[display("{message}")] pub struct S { pub message: String }"#,
        r#"#[derive(derive_more::Display)] pub struct S { pub message: String }"#,
    ),
    (
        "uninlined_positional",
        r#"#[derive(derive_more::Display)] #[display("{}", _0)] pub struct S(pub String);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub String);"#,
    ),
    (
        "named_argument",
        r#"#[derive(derive_more::Display)] #[display("{x}", x = _0)] pub struct S(pub String);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub String);"#,
    ),
    (
        "explicit_index_zero",
        r#"#[derive(derive_more::Display)] #[display("{0}", _0)] pub struct S(pub String);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub String);"#,
    ),
    (
        "raw_ident_field",
        r#"#[derive(derive_more::Display)] #[display("{type}")] pub struct S { pub r#type: String }"#,
        r#"#[derive(derive_more::Display)] pub struct S { pub r#type: String }"#,
    ),
    (
        "empty_format_spec",
        r#"#[derive(derive_more::Display)] #[display("{_0:}")] pub struct S(pub String);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub String);"#,
    ),
    (
        "binary",
        r#"#[derive(derive_more::Binary)] #[binary("{_0:b}")] pub struct S(pub u8);"#,
        r#"#[derive(derive_more::Binary)] pub struct S(pub u8);"#,
    ),
    (
        "lower_exp",
        r#"#[derive(derive_more::LowerExp)] #[lower_exp("{_0:e}")] pub struct S(pub f64);"#,
        r#"#[derive(derive_more::LowerExp)] pub struct S(pub f64);"#,
    ),
    (
        "lower_hex",
        r#"#[derive(derive_more::LowerHex)] #[lower_hex("{_0:x}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::LowerHex)] pub struct S(pub u32);"#,
    ),
    (
        "octal",
        r#"#[derive(derive_more::Octal)] #[octal("{_0:o}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::Octal)] pub struct S(pub u32);"#,
    ),
    (
        "upper_exp",
        r#"#[derive(derive_more::UpperExp)] #[upper_exp("{_0:E}")] pub struct S(pub f64);"#,
        r#"#[derive(derive_more::UpperExp)] pub struct S(pub f64);"#,
    ),
    (
        "upper_hex",
        r#"#[derive(derive_more::UpperHex)] #[upper_hex("{_0:X}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::UpperHex)] pub struct S(pub u32);"#,
    ),
    (
        "pointer_reference",
        r#"#[derive(derive_more::Pointer)] #[pointer("{_0:p}")] pub struct S(pub &'static u32);"#,
        r#"#[derive(derive_more::Pointer)] pub struct S(pub &'static u32);"#,
    ),
    (
        "pointer_box",
        r#"#[derive(derive_more::Pointer)] #[pointer("{_0:p}")] pub struct S(pub Box<u32>);"#,
        r#"#[derive(derive_more::Pointer)] pub struct S(pub Box<u32>);"#,
    ),
    (
        "pointer_generic",
        r#"#[derive(derive_more::Pointer)] #[pointer("{_0:p}")] pub struct S<T>(pub T);"#,
        r#"#[derive(derive_more::Pointer)] pub struct S<T>(pub T);"#,
    ),
    // Generic containers: the `where` clause is the interface.
    (
        "generic_inline",
        r#"#[derive(derive_more::Display)] #[display("{_0}")] pub struct S<T>(pub T);"#,
        r#"#[derive(derive_more::Display)] pub struct S<T>(pub T);"#,
    ),
    (
        "generic_positional",
        r#"#[derive(derive_more::Display)] #[display("{}", _0)] pub struct S<T>(pub T);"#,
        r#"#[derive(derive_more::Display)] pub struct S<T>(pub T);"#,
    ),
    (
        "generic_named_argument",
        r#"#[derive(derive_more::Display)] #[display("{x}", x = _0)] pub struct S<T>(pub T);"#,
        r#"#[derive(derive_more::Display)] pub struct S<T>(pub T);"#,
    ),
    (
        "generic_named_field",
        r#"#[derive(derive_more::Display)] #[display("{message}")] pub struct S<T> { pub message: T }"#,
        r#"#[derive(derive_more::Display)] pub struct S<T> { pub message: T }"#,
    ),
    (
        "generic_lower_hex",
        r#"#[derive(derive_more::LowerHex)] #[lower_hex("{_0:x}")] pub struct S<T>(pub T);"#,
        r#"#[derive(derive_more::LowerHex)] pub struct S<T>(pub T);"#,
    ),
    (
        "lifetime_generic",
        r#"#[derive(derive_more::Display)] #[display("{_0}")] pub struct S<'a>(pub &'a str);"#,
        r#"#[derive(derive_more::Display)] pub struct S<'a>(pub &'a str);"#,
    ),
    (
        "two_derives",
        r#"#[derive(derive_more::Display, derive_more::LowerHex)] #[display("{_0}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::Display, derive_more::LowerHex)] pub struct S(pub u32);"#,
    ),
    (
        "enum_variant",
        r#"#[derive(derive_more::Display)] pub enum S { #[display("{_0}")] A(String), #[display("b {_0}")] B(u32) }"#,
        r#"#[derive(derive_more::Display)] pub enum S { A(String), #[display("b {_0}")] B(u32) }"#,
    ),
    (
        "enum_level_variant_placeholder",
        r#"#[derive(derive_more::Display)] #[display("{_variant}")] pub enum S { A, #[display("r {_0}")] B(u64) }"#,
        r#"#[derive(derive_more::Display)] pub enum S { A, #[display("r {_0}")] B(u64) }"#,
    ),
    (
        "enum_generic_variants",
        r#"#[derive(derive_more::Display)] pub enum S<N> { #[display("{_0}")] T(String), #[display("{_0}")] N(N) }"#,
        r#"#[derive(derive_more::Display)] pub enum S<N> { T(String), N(N) }"#,
    ),
    // Shapes the rule must refuse. Each deletion changes the generated
    // code, so the fixer must leave every one of them alone.
    (
        "refused_self_dot_index",
        r#"#[derive(derive_more::Display)] #[display("{}", self.0)] pub struct S(pub String);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub String);"#,
    ),
    (
        "refused_bound_beside_template",
        r#"#[derive(derive_more::Display)] #[display("{_0}")] #[display(bound(T: Clone))] pub struct S<T>(pub T);"#,
        r#"#[derive(derive_more::Display)] #[display(bound(T: Clone))] pub struct S<T>(pub T);"#,
    ),
    (
        "refused_display_placeholder_under_lower_hex",
        r#"#[derive(derive_more::LowerHex)] #[lower_hex("{_0}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::LowerHex)] pub struct S(pub u32);"#,
    ),
    (
        "refused_debug",
        r#"#[derive(derive_more::Debug)] #[debug("{_0:?}")] pub struct S(pub Vec<u8>);"#,
        r#"#[derive(derive_more::Debug)] pub struct S(pub Vec<u8>);"#,
    ),
    (
        "refused_debug_placeholder_under_display",
        r#"#[derive(derive_more::Display)] #[display("{_0:?}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub u32);"#,
    ),
    (
        "refused_adorned_placeholder",
        r#"#[derive(derive_more::Display)] #[display("{_0:>8}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub u32);"#,
    ),
    (
        "refused_index_past_arguments",
        r#"#[derive(derive_more::Display)] #[display("{1}", _0)] pub struct S(pub String);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub String);"#,
    ),
    (
        "refused_variant_under_replacing_template",
        r#"#[derive(derive_more::Display)] #[display("unknown")] pub enum S { #[display("{_0}")] A(String) }"#,
        r#"#[derive(derive_more::Display)] #[display("unknown")] pub enum S { A(String) }"#,
    ),
    (
        "display_variant_under_wrapping",
        r#"#[derive(derive_more::Display)] #[display("d: {_variant}")] pub enum S { #[display("{_0}")] A(String) }"#,
        r#"#[derive(derive_more::Display)] #[display("d: {_variant}")] pub enum S { A(String) }"#,
    ),
    // Under a wrapping enum template `derive_more` leaves the
    // transparent path, and for `Pointer` it then dereferences: the
    // wrapped form prints the pointee's address, the deleted form the
    // binding's.
    (
        "refused_pointer_variant_under_wrapping",
        r#"#[derive(derive_more::Pointer)] #[pointer("p: {_variant}")] pub enum S { #[pointer("{_0:p}")] A(Box<u32>) }"#,
        r#"#[derive(derive_more::Pointer)] #[pointer("p: {_variant}")] pub enum S { A(Box<u32>) }"#,
    ),
    // Aliasing the placeholder makes a `{_variant}` template replacing.
    (
        "refused_aliased_variant_placeholder",
        r#"#[derive(derive_more::Display)] #[display("{_variant}", _variant = 1)] pub enum S { #[display("{_0}")] A(String) }"#,
        r#"#[derive(derive_more::Display)] #[display("{_variant}", _variant = 1)] pub enum S { A(String) }"#,
    ),
];

/// Shapes the rule declines even though the deletion would be a no-op.
/// Each is a deliberate false negative, not an oversight.
const ACCEPTED_MISSED_DIAGNOSTICS: &[&str] = &[
    // A variant under a wrapping enum-level template. Safe for
    // `Display`, unsafe for `Pointer` (see the case below), and the
    // rule declines the whole shape rather than splitting by trait.
    "display_variant_under_wrapping",
    // `{1}` naming the sole argument is transparent for a concrete
    // field type; the generic form, where the inferred bound would
    // differ, does not compile as written.
    "refused_index_past_arguments",
];

#[test]
#[ignore = "builds the lint, fetches `derive_more`, runs three expansions; see the module docs"]
fn autofix_never_changes_the_generated_code() {
    let fixture = fixture_crate();
    let dir = fixture.path();

    write_source(dir, |case| case.1);
    let before = expand(dir, "as written");
    let source_before = read_source(dir);

    apply_the_real_fix(dir);
    let source_after = read_source(dir);
    let after = expand(dir, "after `cargo dylint --fix`");
    assert_compiles(dir, "the fixed crate");

    // A third expansion, of every attribute deleted by hand, is what
    // lets a declined shape be told from one that never needed fixing.
    write_source(dir, |case| case.2);
    let deleted = expand(dir, "attribute deleted by hand");

    let mut failures = Vec::new();
    for (name, _, _) in CASES {
        let fixed = source_before[*name] != source_after[*name];
        if fixed {
            if before[*name] != after[*name] {
                failures.push(format!(
                    "{name}: the rule deleted the attribute and the generated code changed\n  \
                     before: {}\n  after:  {}",
                    before[*name], after[*name],
                ));
            }
        } else if before[*name] == deleted[*name] && !ACCEPTED_MISSED_DIAGNOSTICS.contains(name) {
            failures.push(format!(
                "{name}: the rule left this alone, but deleting the attribute changes \
                 nothing — either flag it or record it as a deliberate miss",
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

/// A fixture crate depending on the real `derive_more`, materialised
/// fresh so the sources under test are never inherited from a previous
/// run. Only the *build* is shared, through [`shared_target_dir`].
///
/// The manifest is written here rather than through
/// `_utils::build_project` because this fixture needs a real
/// dependency, which the shared builder does not model; the
/// `dylint.toml` still goes through [`fixture_dylint_toml`], so the
/// plugin is discovered exactly as in every other fixture.
fn fixture_crate() -> TempDir {
    let fixture = TempDir::new().expect("create fixture dir");
    let dir = fixture.path();
    fs::create_dir_all(dir.join("src")).expect("create fixture src dir");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"autofix_no_op\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [lib]\npath = \"src/lib.rs\"\n\n\
         [dependencies]\nderive_more = { version = \"2\", features = [\"full\"] }\n\n\
         [workspace]\n",
    )
    .expect("write fixture manifest");
    fs::write(
        dir.join("dylint.toml"),
        fixture_dylint_toml(cargo_manifest_dir()),
    )
    .expect("write fixture dylint.toml");
    fixture
}

fn write_source(dir: &Path, pick: impl Fn(&Case) -> &'static str) {
    let source: String = CASES
        .iter()
        .map(|case| format!("pub mod {} {{ {} }}\n", case.0, pick(case)))
        .collect();
    fs::write(dir.join("src/lib.rs"), source).expect("write scratch source");
}

fn read_source(dir: &Path) -> BTreeMap<String, String> {
    let source = fs::read_to_string(dir.join("src/lib.rs")).expect("read scratch source");
    source
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("pub mod ")?;
            let (name, body) = rest.split_once(' ')?;
            Some((
                name.to_owned(),
                body.split_whitespace().collect::<Vec<_>>().join(" "),
            ))
        })
        .collect()
}

/// Prepare a Cargo invocation inside the fixture, sharing the warmed
/// integration-test target dir so std and the perfectionist plugin are
/// not rebuilt, and with the pinned `cargo-dylint` on `PATH`.
fn cargo(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    let dev_tools = cargo_manifest_dir().join(".dev-tools/bin");
    let path = std::env::var("PATH").unwrap_or_default();
    command
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", shared_target_dir())
        .env("PATH", format!("{}:{path}", dev_tools.display()));
    command
}

/// Run the rule's own autofix over the scratch crate, so the deletion
/// under test is the one the rule actually emits.
fn apply_the_real_fix(dir: &Path) {
    let output = cargo(dir)
        .args([
            "dylint",
            "--fix",
            "--all",
            "--",
            "--lib",
            "--allow-no-vcs",
            "--allow-dirty",
        ])
        .output()
        .expect("run cargo dylint --fix");
    assert!(
        output.status.success(),
        "cargo dylint --fix failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

fn assert_compiles(dir: &Path, what: &str) {
    let output = cargo(dir)
        .args(["check", "--quiet", "--lib"])
        .output()
        .expect("run cargo check");
    assert!(
        output.status.success(),
        "{what} does not compile:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Expand the scratch crate and return the generated code per case.
fn expand(dir: &Path, what: &str) -> BTreeMap<String, String> {
    let output = cargo(dir)
        .args(["rustc", "--quiet", "--lib", "--", "-Zunpretty=expanded"])
        .output()
        .expect("run cargo rustc");
    assert!(
        !output.stdout.is_empty(),
        "expanding the crate {what} produced nothing:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
    split_modules(&String::from_utf8_lossy(&output.stdout))
}

/// Split the expanded crate into one normalised chunk per case module.
fn split_modules(expanded: &str) -> BTreeMap<String, String> {
    let mut modules = BTreeMap::new();
    let mut current: Option<(String, Vec<&str>, isize)> = None;
    for line in expanded.lines() {
        match &mut current {
            None => {
                if let Some(name) = line
                    .strip_prefix("pub mod ")
                    .and_then(|rest| rest.strip_suffix(" {"))
                {
                    current = Some((name.to_owned(), Vec::new(), 1));
                }
            }
            Some((name, body, depth)) => {
                *depth += count(line, '{') - count(line, '}');
                if *depth <= 0 {
                    modules.insert(name.clone(), normalise(&body.join("\n")));
                    current = None;
                } else {
                    body.push(line);
                }
            }
        }
    }
    for (name, _, _) in CASES {
        assert!(modules.contains_key(*name), "case `{name}` did not expand");
    }
    modules
}

fn count(line: &str, brace: char) -> isize {
    line.chars().filter(|found| *found == brace).count() as isize
}

/// Reduce an expanded module to just the code the derive generated:
/// drop the echoed helper attribute, which differs by construction —
/// the whole point is that it is gone — and collapse whitespace so the
/// pretty-printer's line wrapping cannot masquerade as a difference.
fn normalise(expanded: &str) -> String {
    const HELPERS: &[&str] = &[
        "display",
        "debug",
        "binary",
        "lower_exp",
        "lower_hex",
        "octal",
        "pointer",
        "upper_exp",
        "upper_hex",
    ];
    let mut out = String::with_capacity(expanded.len());
    let mut rest = expanded;
    'outer: while let Some(open) = rest.find("#[") {
        let after = &rest[open + "#[".len()..];
        for helper in HELPERS {
            let Some(args) = after.strip_prefix(helper).and_then(|a| a.strip_prefix('(')) else {
                continue;
            };
            // Attribute arguments nest one level at most (`bound(T: Clone)`).
            let mut depth = 1;
            for (index, character) in args.char_indices() {
                depth += match character {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                if depth == 0 && args[index..].starts_with(")]") {
                    out.push_str(&rest[..open]);
                    rest = &args[index + ")]".len()..];
                    continue 'outer;
                }
            }
        }
        out.push_str(&rest[..open + "#[".len()]);
        rest = after;
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
