# perfectionist

`perfectionist` is a [Dylint] library that provides additional
linting rules for Rust projects.

The rules enforce stylistic and structural conventions that go
beyond what `rustc` and `clippy` cover. They concern module
layout, import shape, doc-comment typography, derive ordering,
and similar fine-grained details. Every rule is opinionated and
opt-in per project.

[Dylint]: https://github.com/trailofbits/dylint

## Status

The project is in early development. The catalogue of intended
rules lives under [`planned-rules/`](./planned-rules/). Each
file in that directory is the specification for one lint.
Implemented rules are removed from the directory as they land.
The full index is maintained in
[`planned-rules/README.md`](./planned-rules/README.md).

## Usage

Install Dylint, then add `perfectionist` to your workspace's
`Cargo.toml`:

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

Each lint registers under the `perfectionist` tool namespace.
Suppress a finding at a specific site by name. For example:

```rust
#[allow(perfectionist::unicode_ellipsis_in_comments)]
```

## Configuration

Per-rule configuration is read from `dylint.toml` at the
workspace root. The configuration knobs for each rule are
documented in that rule's planning file. Once a rule is
implemented, the same information is reproduced in its module
documentation.

## Development

The project uses a `justfile` for common tasks:

```sh
just build       # cargo build
just test        # cargo test
just lint        # cargo clippy --all-targets -- -D warnings
just self-lint   # run perfectionist's own lints on its source
just all         # fmt, build, doc, lint, test, and self-lint
```

Contributors implementing a planned rule should read
[`CLAUDE.md`](./CLAUDE.md) for the implementation workflow. The
same file is also exposed under the name `AGENTS.md`.
Cross-cutting conventions, including parser style and lint-name
namespacing, are documented in
[`planned-rules/IMPLEMENTATION_CONVENTIONS.md`](./planned-rules/IMPLEMENTATION_CONVENTIONS.md).

## License

[MIT](./LICENSE.md)
