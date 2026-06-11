// force-host
// no-prefer-dynamic
//
// A miniature stand-in for `clap_derive`. Each derive is inert — it
// emits no tokens, leaving the annotated struct / enum untouched — and
// declares the same helper-attribute set (`clap`, `command`, `arg`,
// `value`, `group`) the real derives do, so a fixture can write
// `#[arg(help = "...")]` / `#[command(about = "...")]` /
// `#[clap(verbatim_doc_comment)]` and still compile. The crate is named
// `clap` (from this file name), so fixtures spell the derives
// `clap::Parser`, matching how `clap_help_no_markdown` recognises them
// by their final path segment.

#![crate_type = "proc-macro"]

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
