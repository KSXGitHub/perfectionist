#![feature(register_tool)]
#![register_tool(perfectionist)]
#![allow(
    unknown_lints,
    dead_code,
    unused_imports,
    reason = "ui fixture"
)]

// Default style is `module`: one `use` per leaf module, items from the
// same module merged, sibling modules on their own lines. Each case
// lives in its own module so the consecutive-`use` runs stay isolated.

// Bad: the same module is split across two `use` statements.
mod same_module_split {
    use std::collections::BTreeMap;
    use std::collections::HashMap;
}

// Bad: one `use` crosses two modules; module style puts each module on
// its own line.
mod cross_module {
    use std::{collections::HashMap, io::Read};
}

// Bad: items pushed down into nested braces below the leaf module.
mod nested_below_leaf {
    use std::collections::{BTreeMap, btree_map::Entry};
}

// Good: already one `use` per leaf module.
mod already_module {
    use std::collections::{BTreeMap, HashMap};
    use std::io::Read;
}

// Good: distinct sibling modules each on their own line.
mod distinct_modules {
    use std::collections::HashMap;
    use std::path::Path;
}

// Good: a lone single-item `use` is already minimal.
mod single_item {
    use std::collections::HashMap;
}

// Suppressed: an `#[allow]` on the enclosing module silences the rule.
#[allow(perfectionist::import_granularity, reason = "ui fixture")]
mod suppressed {
    use std::collections::BTreeMap;
    use std::collections::HashMap;
}

// Declined: a brace carrying a bare `*` (no module path to anchor on)
// is left untouched rather than rewritten.
mod bare_glob {
    use {std::collections::HashMap, *};
}

// Good: under default `respect_visibility`, a `pub use` is not merged
// with a private `use` from the same module.
mod visibility_respected {
    pub use std::collections::BTreeMap;
    use std::collections::HashMap;
}

// Good: under default `respect_cfg_blocks`, a platform-gated import
// stays on its own line rather than merging with an unconditional one.
// `import_grouping` is allowed here: under its default `grouped` style
// the cfg-gated import forms a trailing group, which this fixture
// deliberately places first to exercise granularity, not grouping.
#[allow(perfectionist::import_grouping, reason = "exercises import_granularity")]
mod cfg_respected {
    #[cfg(unix)]
    use std::collections::BTreeMap;
    use std::collections::HashMap;
}

mod a {}
mod b {}

// Bad: a top-level brace grouping bare crate roots is split into one
// `use` per root under module style.
mod top_level_brace {
    use {a, b};
}

fn main() {}
