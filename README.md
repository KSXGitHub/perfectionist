# perfectionist

Additional linting rules for Rust projects.

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

Prerequisites:

- [Rustup](https://rustup.rs/)
- [Just](https://github.com/casey/just/)
- [cargo-dylint](https://github.com/trailofbits/dylint/tree/master/cargo-dylint)
- [dylint-link](https://github.com/trailofbits/dylint/tree/master/dylint-link)

Run the following command to check everything:

```sh
just all
```

## License

[MIT](https://github.com/KSXGitHub/perfectionist/blob/master/LICENSE.md) © [Hoàng Văn Khải](https://github.com/KSXGitHub/).
