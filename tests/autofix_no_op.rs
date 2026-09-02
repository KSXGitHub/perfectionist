//! Proves the `redundant_derive_more_forward_template` autofix is a
//! no-op, against the real `derive_more`.
//!
//! The rule's suggestion is `MachineApplicable`, so applying it must
//! leave the generated code *identical* — same body, same `where`
//! clause. Reading the expander is not enough to establish that: the
//! deref injection in `additional_deref_args` and the bound assembly in
//! `generate_bounds` both look decisive in isolation and are not, and
//! reasoning about them produced a wrong answer in each direction
//! before this test existed.
//!
//! So each case is compiled twice — once as the user wrote it, once
//! with the attribute deleted exactly as the fix deletes it — and the
//! two macro expansions are compared. [`FLAGGED`] must expand
//! identically; [`REFUSED`] must not, which is what makes each bail-out
//! earn its place instead of being cargo-culted forward.
//!
//! Ignored by default: it fetches `derive_more` and shells out to two
//! full expansions, which does not belong in the gating suite. Run it
//! when the trigger changes:
//!
//! ```text
//! cargo test --test autofix_no_op -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `(name, as written, with the attribute deleted)`.
type Case = (&'static str, &'static str, &'static str);

/// Shapes the rule flags. Deleting the attribute must change nothing.
const FLAGGED: &[Case] = &[
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
    // `Pointer` takes the transparent path for a lone placeholder, so
    // `additional_deref_args` never applies to a template flagged here.
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
    // Generic containers: the `where` clause is the interface, and it
    // has to survive the deletion unchanged.
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
        r#"#[derive(derive_more::Display, derive_more::LowerHex)] #[display("{_0}")] #[lower_hex("{_0:x}")] pub struct S(pub u32);"#,
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
        "enum_variant_under_wrapping_template",
        r#"#[derive(derive_more::Display)] #[display("w: {_variant}")] pub enum S { #[display("{_0}")] A(String) }"#,
        r#"#[derive(derive_more::Display)] #[display("w: {_variant}")] pub enum S { A(String) }"#,
    ),
    (
        "enum_generic_variants",
        r#"#[derive(derive_more::Display)] pub enum S<N> { #[display("{_0}")] T(String), #[display("{_0}")] N(N) }"#,
        r#"#[derive(derive_more::Display)] pub enum S<N> { T(String), N(N) }"#,
    ),
];

/// Shapes the rule refuses. Deleting the attribute must change the
/// generated code — otherwise the bail-out is dead weight.
const REFUSED: &[Case] = &[
    (
        "self_dot_index_rewrites_the_body",
        r#"#[derive(derive_more::Display)] #[display("{}", self.0)] pub struct S(pub String);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub String);"#,
    ),
    (
        "self_dot_name_rewrites_the_body",
        r#"#[derive(derive_more::Display)] #[display("{}", self.message)] pub struct S { pub message: String }"#,
        r#"#[derive(derive_more::Display)] pub struct S { pub message: String }"#,
    ),
    (
        "self_dot_on_generic_adds_a_bound",
        r#"#[derive(derive_more::Display)] #[display("{}", self.0)] pub struct S<T>(pub T);"#,
        r#"#[derive(derive_more::Display)] pub struct S<T>(pub T);"#,
    ),
    (
        "bound_beside_template_is_dropped",
        r#"#[derive(derive_more::Display)] #[display("{_0}")] #[display(bound(T: Clone))] pub struct S<T>(pub T);"#,
        r#"#[derive(derive_more::Display)] #[display(bound(T: Clone))] pub struct S<T>(pub T);"#,
    ),
    (
        "display_placeholder_under_lower_hex",
        r#"#[derive(derive_more::LowerHex)] #[lower_hex("{_0}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::LowerHex)] pub struct S(pub u32);"#,
    ),
    (
        "debug_does_not_default_to_a_forward",
        r#"#[derive(derive_more::Debug)] #[debug("{_0:?}")] pub struct S(pub Vec<u8>);"#,
        r#"#[derive(derive_more::Debug)] pub struct S(pub Vec<u8>);"#,
    ),
    (
        "debug_placeholder_under_display",
        r#"#[derive(derive_more::Display)] #[display("{_0:?}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub u32);"#,
    ),
    (
        "adorned_placeholder",
        r#"#[derive(derive_more::Display)] #[display("{_0:>8}")] pub struct S(pub u32);"#,
        r#"#[derive(derive_more::Display)] pub struct S(pub u32);"#,
    ),
    (
        "index_past_the_argument_list",
        r#"#[derive(derive_more::Display)] #[display("{1}", _0)] pub struct S<T>(pub T);"#,
        r#"#[derive(derive_more::Display)] pub struct S<T>(pub T);"#,
    ),
    (
        "variant_under_replacing_enum_template",
        r#"#[derive(derive_more::Display)] #[display("unknown")] pub enum S { #[display("{_0}")] A(String) }"#,
        r#"#[derive(derive_more::Display)] #[display("unknown")] pub enum S { A(String) }"#,
    ),
    (
        "non_transparent_pointer_template",
        r#"#[derive(derive_more::Pointer)] #[pointer("p {_0:p}")] pub struct S(pub Box<u32>);"#,
        r#"#[derive(derive_more::Pointer)] pub struct S(pub Box<u32>);"#,
    ),
];

#[test]
#[ignore = "fetches `derive_more` and runs two full expansions; see the module docs"]
fn autofix_never_changes_the_generated_code() {
    let dir = scratch_crate();
    let as_written = expand(&dir, Variant::AsWritten);
    let fixed = expand(&dir, Variant::Fixed);

    let mut failures = Vec::new();
    for (name, _, _) in FLAGGED {
        let (before, after) = (&as_written[*name], &fixed[*name]);
        if before != after {
            failures.push(format!(
                "{name}: the fix changed the generated code\n  as written: {before}\n  fixed:      {after}",
            ));
        }
    }
    for (name, _, _) in REFUSED {
        if as_written[*name] == fixed[*name] {
            failures.push(format!(
                "{name}: deleting the attribute changes nothing, so the rule's \
                 refusal to flag it is unnecessary",
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

enum Variant {
    /// The attribute as a user writes it.
    AsWritten,
    /// The attribute deleted, exactly as the autofix deletes it.
    Fixed,
}

fn cases() -> impl Iterator<Item = &'static Case> {
    FLAGGED.iter().chain(REFUSED)
}

/// A throwaway crate depending on the real `derive_more`, sharing this
/// workspace's `target/` so the dependency is fetched at most once.
fn scratch_crate() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/autofix-no-op");
    fs::create_dir_all(dir.join("src")).expect("create scratch crate");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"autofix_no_op\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [dependencies]\nderive_more = { version = \"2\", features = [\"full\"] }\n\n\
         [workspace]\n",
    )
    .expect("write scratch manifest");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("rust-toolchain"),
        dir.join("rust-toolchain"),
    )
    .expect("pin the scratch crate to this toolchain");
    dir
}

/// Expand every case in one compilation, returning the generated code
/// per case.
fn expand(dir: &Path, variant: Variant) -> BTreeMap<String, String> {
    let source: String = cases()
        .map(|(name, as_written, fixed)| {
            let body = match variant {
                Variant::AsWritten => as_written,
                Variant::Fixed => fixed,
            };
            format!("pub mod {name} {{ {body} }}\n")
        })
        .collect();
    fs::write(dir.join("src/lib.rs"), source).expect("write scratch source");

    let output = Command::new(env!("CARGO"))
        .current_dir(dir)
        .args(["rustc", "--quiet", "--", "-Zunpretty=expanded"])
        .output()
        .expect("run cargo");
    assert!(
        !output.stdout.is_empty(),
        "expansion produced nothing:\n{}",
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
    for (name, _, _) in cases() {
        assert!(modules.contains_key(*name), "case `{name}` did not expand");
    }
    modules
}

fn count(line: &str, brace: char) -> isize {
    line.chars().filter(|found| *found == brace).count() as isize
}

/// Reduce an expanded module to just the code the derive generated:
/// drop the echoed helper attribute (which differs by construction —
/// the whole point is that it is gone) and collapse whitespace, so the
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
