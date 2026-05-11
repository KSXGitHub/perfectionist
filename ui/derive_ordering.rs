// The default `style = "preserve"` makes the rule a no-op. No
// fixture in this file should produce a diagnostic; the
// configuration-driven styles are exercised under
// `ui-toml/derive_ordering/`.

#[derive(Debug, Clone, Copy)]
struct _DeliberatelyOutOfOrder;

#[derive(Clone, Copy, Debug)]
struct _AlreadyAlphabetical;

#[derive(Debug)]
struct _OneTrait;

#[derive()]
struct _NoTraits;

fn main() {}
