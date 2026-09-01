// force-host
// no-prefer-dynamic
//
// A miniature stand-in for `derive_more`'s formatting derives. Each
// derive is inert — it emits no tokens, leaving the annotated struct /
// enum untouched — and declares the same helper-attribute set the real
// derives do, so a fixture can write `#[display("{_0}")]`,
// `#[lower_hex("{_0:x}")]`, `#[debug("{_0:?}")]`, ... and still
// compile. The crate is named `derive_more` (from this file name), so
// fixtures spell the derives `derive_more::Display` or import them,
// matching how `redundant_derive_more_forward_template` recognises
// them by their final path segment.

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

macro_rules! inert_derive {
    ($fn_name:ident, $trait_name:ident) => {
        #[proc_macro_derive(
            $trait_name,
            attributes(
                binary, debug, display, lower_exp, lower_hex, octal, pointer, upper_exp,
                upper_hex
            )
        )]
        pub fn $fn_name(_input: TokenStream) -> TokenStream {
            TokenStream::new()
        }
    };
}

inert_derive!(binary, Binary);
inert_derive!(debug, Debug);
inert_derive!(display, Display);
inert_derive!(lower_exp, LowerExp);
inert_derive!(lower_hex, LowerHex);
inert_derive!(octal, Octal);
inert_derive!(pointer, Pointer);
inert_derive!(upper_exp, UpperExp);
inert_derive!(upper_hex, UpperHex);
