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

fn main() {}
