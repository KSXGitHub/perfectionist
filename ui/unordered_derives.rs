// The rule ships disabled by default
// (`DEFAULT_STATE = DefaultState::Disabled` in
// `src/rules/unordered_derives.rs`), so without a `dylint.toml` that
// enables it the pass never registers and nothing in this file
// produces a diagnostic. The configuration-driven styles are
// exercised under `ui-toml/unordered_derives/`, each with its own
// `dylint.toml` that opts the rule in.

#[derive(Debug, Clone, Copy)]
struct _DeliberatelyOutOfOrder;

#[derive(Clone, Copy, Debug)]
struct _AlreadyAlphabetical;

#[derive(Debug)]
struct _OneTrait;

#[derive()]
struct _NoTraits;

fn main() {}
