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

Each lint registers under the `perfectionist` tool namespace. Suppress a finding at a specific site by name. For example:

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

## Configuration

Per-rule configuration is read from `dylint.toml` at the workspace root. The configuration knobs for each rule are documented in that rule's planning file. Once a rule is implemented, the same information is reproduced in its module documentation.

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
