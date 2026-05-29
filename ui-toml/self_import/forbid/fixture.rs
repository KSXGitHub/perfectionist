#![allow(unused_imports, unused, reason = "ui fixture deliberately leaves imports unused")]

mod defs {
    pub mod inner {
        pub struct Baz;
        pub struct Qux;
        pub mod sub {
            pub struct Item;
        }
    }
    pub mod sibling {
        pub struct Thing;
    }
    pub mod r#match {
        pub struct Token;
    }
}

// `use module::{self};` collapses to the bare module import.
mod brace_self {
    use crate::defs::inner::{self};
}

// `use module::{self, X};` splits into two statements at the item root.
mod brace_self_item {
    use crate::defs::inner::{self, Baz};
}

// The `self` leaf's rename is preserved on the bare module import.
mod brace_self_renamed {
    use crate::defs::inner::{self as renamed};
}

// The `module::self` form (only valid inside a brace list) drops `self`.
mod trailing_self_in_braces {
    use crate::defs::{inner::self};
}

// A nested sole-`self` group folds to the bare module path in place.
mod nested_self_sole {
    use crate::defs::{inner::{self}, sibling};
}

// A nested `{self, X}` group expands into sibling brace entries.
mod nested_self_item {
    use crate::defs::{inner::{self, Baz}, sibling};
}

// A raw-identifier module renders with its `r#` prefix intact.
mod raw_ident {
    use crate::defs::r#match::{self};
}

// The split preserves the item's attributes on both statements.
mod attr_split {
    #[allow(unused_imports, reason = "self_import attribute-preservation fixture")]
    use crate::defs::inner::{self, Baz};
}

// The split preserves visibility on both synthesised statements.
mod pub_split {
    pub use crate::defs::inner::{self, Baz};
}

// A `{self, *}` group splits the module import out from the glob.
mod glob_with_self {
    use crate::defs::inner::{self, *};
}

// A leading `::` (absolute path) is preserved in the rewrite.
mod rooted_path {
    use ::std::collections::{self};
}

// A cfg-gated import is still linted: the rule re-parses the source,
// which keeps `#[cfg(...)]` gates intact, so the `use` is seen even
// though the cfg would strip it from the build (`any()` is always false).
mod cfg_gated {
    #[cfg(any())]
    use crate::defs::inner::{self};
}

// A brace group whose non-`self` sibling carries its own nested `self`
// is left for a later pass: rewriting it now would emit a suggestion
// overlapping the inner one and still leave a `self` import behind, so
// only the inner `self` is flagged here (the outer follows once the
// inner is fixed).
mod nested_self_in_sibling {
    use crate::defs::inner::{self, sub::{self}};
}

// A prefix-less brace group (`{self as e, sibling}`) is left alone — it
// has no path to render — but it must NOT suppress the *outer* group's
// own `self`: that one has a real prefix and is still split out. (The
// deferral above only applies when a sibling carries a `self` the rule
// would actually rewrite.)
mod empty_prefix_sibling {
    use crate::defs::{self as d, {self as e, sibling}};
}

// An outline module (a separate source file) is linted too. The rule
// re-parses each module source file (out-of-line `mod foo;` bodies are
// not in the crate-root AST), so a separate file's `use` is reached just
// like the crate root's.
#[path = "auxiliary/outline.rs"]
mod outline;

// A bare module import is already compliant — no diagnostic.
mod clean {
    use crate::defs::sibling;
}

fn main() {}
