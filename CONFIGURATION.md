# Configuration

## Crate-wide configuration: the `[perfectionist]` table

The top-level `[perfectionist]` table of `dylint.toml` controls which rules' passes are installed. It has two keys; both are optional:

| Key       | Type                                             | Meaning                                                                 |
| --------- | ------------------------------------------------ | ----------------------------------------------------------------------- |
| `enable`  | array of rule names or `{ name, reason }` tables | Force-on the named rules, even if their per-rule default is `disabled`. |
| `disable` | array of rule names or `{ name, reason }` tables | Force-off the named rules, even if their per-rule default is `enabled`. |

Rule names are unqualified — drop the `perfectionist::` prefix that appears in attributes and CLI flags. The optional `reason` field on each entry is preserved for human readers of `dylint.toml` and has no runtime effect.

Unknown rule names are silently ignored. Listing the same rule under both `enable` and `disable` is a fatal config error.

See [CONTROLLING_RULES.md](./CONTROLLING_RULES.md) for the broader picture of how `enable` / `disable` compose with lint-level attributes and `DYLINT_RUSTFLAGS`.

## Per-rule configuration: `["perfectionist::<rule>"]` tables

Each rule has its own configuration table under its full namespaced name, e.g.:

```toml
["perfectionist::exhaustive_error_enums"]
require_for = "pub_crate"
```

The header is quoted because a TOML bare key admits only `A-Za-z0-9_-`; the `::` in the namespaced name makes `[perfectionist::exhaustive_error_enums]` a syntax error.

The available knobs for each rule are documented in that rule's planning file. Once a rule is implemented, the same information is reproduced in its module documentation and in [`rules/`](./rules/).
