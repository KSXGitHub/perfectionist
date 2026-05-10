# perfectionist

Additional linting rules for Rust projects, distributed as a
[Dylint] library.

`perfectionist` enforces stylistic and structural conventions that
go beyond what `rustc` and `clippy` cover — module layout, import
shape, doc-comment typography, derive ordering, and similar
fine-grained details. Rules are designed to be opinionated but
opt-in per project.

[Dylint]: https://github.com/trailofbits/dylint

## Status

Early development. The catalogue of intended rules lives under
[`planned-rules/`](./planned-rules/); each file is the spec for
one lint. Implemented rules are removed from that directory as
they land. See [`planned-rules/README.md`](./planned-rules/README.md)
for the full index.

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

Suppress a lint at a site with the namespaced name, for example:

```rust
#[allow(perfectionist::unicode_ellipsis_in_comments)]
```

## Configuration

Per-rule configuration is read from `dylint.toml` at the workspace
root. The configuration knobs for each rule are documented in that
rule's planning file (or, once implemented, in its module
documentation).

## Development

The project uses a `justfile` for common tasks:

```sh
just build       # cargo build
just test        # cargo test
just lint        # cargo clippy --all-targets -- -D warnings
just self-lint   # run perfectionist's own lints on its source
just all         # fmt + build + doc + lint + test + self-lint
```

Contributors implementing a planned rule should read
[`CLAUDE.md`](./CLAUDE.md) (also exposed as `AGENTS.md`) for the
implementation workflow, and
[`planned-rules/IMPLEMENTATION_CONVENTIONS.md`](./planned-rules/IMPLEMENTATION_CONVENTIONS.md)
for cross-cutting conventions (parser style, lint-name
namespacing).

## License

[MIT](./LICENSE.md)
