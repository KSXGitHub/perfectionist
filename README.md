# perfectionist

Additional linting rules for Rust projects.

## Rules

See [the homepage](https://KSXGitHub.github.io/perfectionist/) for all the rules implemented so far.

See [`planned-rules/`](https://github.com/KSXGitHub/perfectionist/tree/master/planned-rules) for all the rules not yet implemented.

## Usage

Install [Dylint](https://github.com/trailofbits/dylint), then add `perfectionist` to your workspace's `dylint.toml`:

```toml
[workspace.metadata.dylint]
libraries = [
    { git = "https://github.com/KSXGitHub/perfectionist" },
]
```

Run the lints:

```sh
cargo dylint --all -- --all-targets
```

Each lint registers under the `perfectionist` tool namespace. There are two independent knobs for controlling a rule:

**1. Lint level (per site).** Use rustc's `#[allow]` / `#[warn]` / `#[deny]` / `#[forbid]` / `#[expect]` attributes at the call site or crate root to change how a finding is reported. For example:

```rust
#[expect(perfectionist::unicode_ellipsis_in_comments, reason = "3 ASCII dots would be incorrect here")]
```

<details>
<summary>Boilerplate to make <code>#[expect(perfectionist::…)]</code> compile</summary>

For the `perfectionist::*` path in the attribute above to be accepted, the compiler needs to know about the `perfectionist` tool namespace. Add the following to your crate root (e.g. `src/lib.rs` or `src/main.rs`):

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

</details>

To escalate a rule project-wide, add an attribute to `src/lib.rs` (`#![deny(perfectionist::single_letter_let_binding)]`) or set `DYLINT_RUSTFLAGS=-D perfectionist::<rule>` in the environment.

**2. Rule registration (project-wide).** Each rule is either *enabled* (its pass runs) or *disabled* (its pass is never installed, so it produces no diagnostics at all). Most rules are enabled by default; a few — currently only `non_exhaustive_error` — ship disabled and require an explicit opt-in. Flip the registration state via the crate-wide `[perfectionist]` table in `dylint.toml`:

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

The per-rule default (enabled / disabled) is documented in each rule's page in [`rules/`](https://github.com/KSXGitHub/perfectionist/tree/master/rules) as **Default state**.

## Configuration

### Crate-wide configuration: the `[perfectionist]` table

The top-level `[perfectionist]` table of `dylint.toml` controls which rules' passes are installed. It has two keys; both are optional:

| Key | Type | Meaning |
| --- | --- | --- |
| `enable` | array of rule names or `{ name, reason }` tables | Force-on the named rules, even if their per-rule default is `disabled`. |
| `disable` | array of rule names or `{ name, reason }` tables | Force-off the named rules, even if their per-rule default is `enabled`. |

Rule names are unqualified — drop the `perfectionist::` prefix that appears in attributes and CLI flags. The optional `reason` field on each entry is preserved for human readers of `dylint.toml` and has no runtime effect.

### Per-rule configuration: `[perfectionist::<rule>]` tables

Each rule has its own configuration table under its full namespaced name, e.g.:

```toml
[perfectionist::non_exhaustive_error]
require_for = "pub_crate"
```

The available knobs for each rule are documented in that rule's planning file. Once a rule is implemented, the same information is reproduced in its module documentation and in [`rules/`](https://github.com/KSXGitHub/perfectionist/tree/master/rules).

## Development

**Prerequisites:**
* [Rustup](https://rustup.rs/)
* [Just](https://github.com/casey/just/)

`cargo-dylint` and `dylint-link` are not part of the cargo dependency graph but their ABI is coupled to it. They're installed into a workspace-local `.dev-tools/` directory (pinned to the `dylint_linting` version in `Cargo.lock`) by:

```sh
just install-dev-tools
```

Run this once after cloning, and again whenever `dylint_linting` is bumped. Every other `just` recipe prepends `.dev-tools/bin` to `PATH`, so subsequent commands use the pinned binaries automatically.

Run the following command to check everything:

```sh
just all
```

## License

[MIT](https://github.com/KSXGitHub/perfectionist/blob/master/LICENSE.md) © [Hoàng Văn Khải](https://github.com/KSXGitHub/).
