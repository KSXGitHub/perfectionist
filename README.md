# perfectionist

Additional linting rules for Rust projects.

## Rules

See [the homepage](https://KSXGitHub.github.io/perfectionist/) or [the `rules/` directory](https://github.com/KSXGitHub/perfectionist/tree/master/rules) for all the rules implemented so far.

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

## Controlling rules

Each lint registers under the `perfectionist` tool namespace. See [CONTROLLING_RULES.md](https://github.com/KSXGitHub/perfectionist/blob/master/CONTROLLING_RULES.md) for how to change a rule's level per call site or project-wide, and how to enable or disable rules globally.

## Configuration

`dylint.toml` accepts a crate-wide `[perfectionist]` table and per-rule `["perfectionist::<rule>"]` tables. See [CONFIGURATION.md](https://github.com/KSXGitHub/perfectionist/blob/master/CONFIGURATION.md) for the full schema.

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

## Frequently Asked Questions

### Why does this code base look so ugly?

This code base is almost entirely AI-generated. Or in other words, vibe-coded.

### Why vibe-code?

I needed a way to automatically force the AI agents I use to comply with a certain code style, reducing the time I'll have to spend micro-managing and reviewing AI-generated code. I cannot rely on them interpreting the rules in the Markdown guides correctly, and they often don't comply with the rules completely. Hence, I looked at the code style guides and thought: "Hmm, some of these rules can be coded into a program!" and so this program was born.

I needed encode these rules quick, so I chose vibe-coding.

The code quality of this code base is not as important as the code quality this code base will enforce upon others. This code base is, after all, scaffholding.

## License

[MIT](https://github.com/KSXGitHub/perfectionist/blob/master/LICENSE.md) © [Hoàng Văn Khải](https://github.com/KSXGitHub/).
