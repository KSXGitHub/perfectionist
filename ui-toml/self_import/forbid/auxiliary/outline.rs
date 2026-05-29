// Loaded as an outline module from `fixture.rs` via `#[path]`. The rule
// runs pre-expansion, where an outline `mod outline;` is still
// `ModKind::Unloaded` in the crate-root AST, so a crate-root-only walk
// would never see this file. The diagnostic below therefore only appears
// if the pass descends through `check_item` as the module loads — this
// fixture guards against regressing to a single `check_crate` walk.
use crate::defs::inner::{self, Baz};
