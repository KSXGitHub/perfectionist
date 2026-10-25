//! A miniature stand-in for clap's derive macros, so the fixture can
//! trigger `perfectionist::clap_help_markdown` without the registry
//! crate: the rule recognises the derives by their final path
//! segment, and this crate is named `clap`, so a fixture spells them
//! `clap::Parser` exactly as it would with the real one.
//!
//! Each derive is inert, emitting no tokens, and declares the same
//! helper attributes the real ones do, so a fixture may write
//! `#[arg(help = "...")]` and still compile.

extern crate proc_macro;

use proc_macro::TokenStream;

macro_rules! inert_derive {
    ($fn_name:ident, $trait_name:ident) => {
        #[proc_macro_derive($trait_name, attributes(clap, command, arg, value, group))]
        pub fn $fn_name(_input: TokenStream) -> TokenStream {
            TokenStream::new()
        }
    };
}

inert_derive!(parser, Parser);
inert_derive!(args, Args);
inert_derive!(subcommand, Subcommand);
inert_derive!(value_enum, ValueEnum);
inert_derive!(command_factory, CommandFactory);
