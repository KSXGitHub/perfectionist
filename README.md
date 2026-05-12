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

## Configuration

Per-rule configuration is read from `dylint.toml` at the workspace root. The configuration knobs for each rule are documented in that rule's planning file. Once a rule is implemented, the same information is reproduced in its module documentation.

## Development

**Prerequisites:**
* [Rustup](https://rustup.rs/)
* [Just](https://github.com/casey/just/)
* [cargo-dylint](https://github.com/trailofbits/dylint/tree/master/cargo-dylint) and [dylint-link](https://github.com/trailofbits/dylint/tree/master/dylint-link), at the same major version as the `dylint_linting` dependency pinned in `Cargo.lock`.

The dylint tools are not managed by cargo's dependency graph, so they have to be installed separately and kept in sync with the workspace. After cloning, and whenever `dylint_linting` is bumped, run:

```sh
just install-dev-tools
```

This installs the matching version of both binaries (via `cargo install --locked --force`). `just warmup-integration-tests` and `just test` start with a preflight check that fails fast with a pointer to this recipe if the local versions drift out of sync.

Run the following command to check everything:

```sh
just all
```

## License

[MIT](https://github.com/KSXGitHub/perfectionist/blob/master/LICENSE.md) © [Hoàng Văn Khải](https://github.com/KSXGitHub/).
