// force-host
// no-prefer-dynamic
//
// A miniature stand-in for `clap_derive`'s `default_value_t` expansion.
// The derive emits `const _: () = { let <one-letter> = <expr>; };`
// where the synthesised `let` binding identifier carries a *user*
// span (the span of the attribute identifier) rather than a call-site
// span. That span pattern is what makes `clap_derive`'s expansion
// slip past rustc's built-in `report_in_external_macro: false` check
// in `perfectionist::single_letter_let_binding`.

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

#[proc_macro_derive(SynthBinding, attributes(synth_default))]
pub fn synth_binding(input: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    // Locate the first `#[synth_default = ...]` attribute and remember
    // the `synth_default` identifier's span. That span is the
    // user-source position the synthesised binding will inherit.
    let attr_span = find_attr_span(&tokens)
        .expect("`#[derive(SynthBinding)]` requires a `#[synth_default = ..]`");

    // `s` carries the user-source span of the `synth_default` attribute
    // identifier — the same span shape `clap_derive` attaches to its
    // own synthesised `let` binding when expanding `default_value_t`.
    let s_ident = Ident::new("s", attr_span);
    let call_site = Span::call_site();

    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(s_ident.clone()),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Literal(Literal::u32_unsuffixed(1)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Ident(s_ident),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);

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

fn find_attr_span(tokens: &[TokenTree]) -> Option<Span> {
    for window in tokens.windows(2) {
        let (hash, group) = (&window[0], &window[1]);
        let is_hash = match hash {
            TokenTree::Punct(p) => p.as_char() == '#',
            _ => false,
        };
        if !is_hash {
            continue;
        }
        let group = match group {
            TokenTree::Group(g) if g.delimiter() == Delimiter::Bracket => g,
            _ => continue,
        };
        for inner in group.stream() {
            if let TokenTree::Ident(ident) = inner {
                if ident.to_string() == "synth_default" {
                    return Some(ident.span());
                }
            }
        }
    }
    None
}
