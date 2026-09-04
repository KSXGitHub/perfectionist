// force-host
// no-prefer-dynamic
//
// A miniature stand-in for the derive macros that emit `core::`-rooted
// paths spanned at the user's own source, the way `syn`'s
// `quote_spanned!` does so that a downstream type error points at a
// field the user can actually edit.
//
// The whole path is stamped with the span of an attribute the user
// wrote, so the `core` token `core_instead_of_std` would rewrite claims
// to be user-written and slips past rustc's
// `report_in_external_macro: false` filter. Only the guard on the
// enclosing item's span stops the rule from offering to rewrite a path
// that exists nowhere in the user's source.
//
// This derive lives here rather than in `ui/auxiliary/` because the
// rule is inactive by default: its regression fixture has to travel
// with a `dylint.toml` that enables the rule, which puts it under
// `ui-toml/` and out of reach of the shared aux crate.

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};

/// `#[derive(SynthCorePath)]` + `#[synth_core_path]` →
/// `const _: () = { type _Synth = core::num::Wrapping<u8>; };` where
/// every token of the `core::num::Wrapping` path inherits the user-span
/// of `synth_core_path`.
#[proc_macro_derive(SynthCorePath, attributes(synth_core_path))]
pub fn synth_core_path(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_core_path")
        .expect("`#[derive(SynthCorePath)]` requires a `#[synth_core_path]`");
    let call_site = Span::call_site();

    // Every token of the path carries the user span, so the path's own
    // span resolves to real source and a span-only filter sees nothing
    // to suppress.
    let at_attr = |mut tree: TokenTree| {
        tree.set_span(attr_span);
        tree
    };
    let mut path = TokenStream::new();
    path.extend([
        at_attr(TokenTree::Ident(Ident::new("core", attr_span))),
        at_attr(TokenTree::Punct(Punct::new(':', Spacing::Joint))),
        at_attr(TokenTree::Punct(Punct::new(':', Spacing::Alone))),
        at_attr(TokenTree::Ident(Ident::new("num", attr_span))),
        at_attr(TokenTree::Punct(Punct::new(':', Spacing::Joint))),
        at_attr(TokenTree::Punct(Punct::new(':', Spacing::Alone))),
        at_attr(TokenTree::Ident(Ident::new("Wrapping", attr_span))),
        at_attr(TokenTree::Punct(Punct::new('<', Spacing::Alone))),
        at_attr(TokenTree::Ident(Ident::new("u8", attr_span))),
        at_attr(TokenTree::Punct(Punct::new('>', Spacing::Alone))),
    ]);

    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("type", call_site)),
        TokenTree::Ident(Ident::new("_Synth", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
    ]);
    body.extend(path);
    body.extend([TokenTree::Punct(Punct::new(';', Spacing::Alone))]);

    // An anonymous `const _` anchor, so the derive can be applied to
    // several types in one crate without colliding.
    let mut out = TokenStream::new();
    out.extend([
        TokenTree::Ident(Ident::new("const", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Brace, body)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);
    out
}

/// The span of the `name` identifier in a `#[name]` / `#[name = ..]`
/// attribute on the derived item, which is a span in the *user's*
/// source.
fn find_attr_span(input: TokenStream, name: &str) -> Option<Span> {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    for window in tokens.windows(2) {
        let (hash, group) = (&window[0], &window[1]);
        let is_hash = match hash {
            TokenTree::Punct(punct) => punct.as_char() == '#',
            _ => false,
        };
        if !is_hash {
            continue;
        }
        let group = match group {
            TokenTree::Group(group) if group.delimiter() == Delimiter::Bracket => group,
            _ => continue,
        };
        for inner in group.stream() {
            if let TokenTree::Ident(ident) = inner {
                if ident.to_string() == name {
                    return Some(ident.span());
                }
            }
        }
    }
    None
}
