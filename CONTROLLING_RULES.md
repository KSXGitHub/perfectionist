# Controlling rules

Each lint registers under the `perfectionist` tool namespace. There are two independent knobs for controlling a rule:

## 1. Lint level (per site)

Use rustc's `#[allow]` / `#[warn]` / `#[deny]` / `#[forbid]` / `#[expect]` attributes at the call site or crate root to change how a finding is reported. For example:

```rust
#[expect(perfectionist::unicode_ellipsis_in_comments, reason = "3 ASCII dots would be incorrect here")]
```

To escalate a rule project-wide, add an attribute to `src/lib.rs` (`#![deny(perfectionist::single_letter_let_binding)]`) or set `DYLINT_RUSTFLAGS=-D perfectionist::<rule>` in the environment.

For any `#[allow / warn / deny / forbid / expect(perfectionist::…)]` attribute *in your source* to be accepted by rustc, the compiler needs to know about the `perfectionist` tool namespace. Add the following to your crate root (e.g. `src/lib.rs` or `src/main.rs`):

```rust
#![cfg_attr(dylint_lib = "perfectionist", feature(register_tool))]
#![cfg_attr(dylint_lib = "perfectionist", register_tool(perfectionist))]
```

Both attributes are gated on `cfg(dylint_lib = "perfectionist")` so they only take effect when the dylint driver is loading this library; regular `cargo build` and `cargo check` runs ignore them and don't need a nightly toolchain.

Then declare the `dylint_lib` cfg in `Cargo.toml` so `cargo check` doesn't warn about an unexpected cfg name:

```toml
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(dylint_lib, values("perfectionist"))'] }
```

The `DYLINT_RUSTFLAGS=-D perfectionist::<rule>` form mentioned above bypasses source-level attributes (rustc receives the flag straight on its command line), so it does not need this boilerplate.

## 2. Rule registration (project-wide)

Each rule is either *enabled* (its pass runs) or *disabled* (its pass is never installed, so it produces no diagnostics at all). Most rules are enabled by default; a few — currently only `non_exhaustive_error` — ship disabled and require an explicit opt-in. Flip the registration state via the crate-wide `[perfectionist]` table in `dylint.toml`:

```toml
[perfectionist]
enable = ["non_exhaustive_error"]
disable = ["arc_rc_clone"]
```

Each entry can also be an inline table carrying a `reason` for the human reading the config later:

```toml
[perfectionist]
enable = [{ name = "non_exhaustive_error", reason = "we publish libraries and care about SemVer surface" }]
disable = [
    "arc_rc_clone",
    { name = "single_letter_closure_param", reason = "we use single-letter binders in math-heavy code" },
]
```

Bare strings and inline `{ name, reason }` tables can be intermixed inside a single literal `enable = [...]` (or `disable = [...]`) array. The array-of-tables form is an alternative syntax for the same data — pick one form per key:

```toml
[[perfectionist.enable]]
name = "non_exhaustive_error"
reason = "we publish libraries and care about SemVer surface"

[[perfectionist.disable]]
name = "arc_rc_clone"
```

(TOML rejects mixing `enable = [...]` and `[[perfectionist.enable]]` in the same file as a duplicate-key error, so a config uses one or the other for each key.)

The two knobs compose: `disable` skips the rule's pass entirely, so its level is moot; `enable` installs the pass, and the level is then whatever rustc resolves from the per-site attributes (or the rule's default `Warn`). Listing the same rule under both `enable` and `disable` is a config error.

The per-rule default (enabled / disabled) is documented in each rule's page in [`rules/`](./rules/) as **Default state**.
