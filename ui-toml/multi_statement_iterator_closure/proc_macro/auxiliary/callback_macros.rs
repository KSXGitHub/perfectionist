// force-host
// no-prefer-dynamic
//
// Two proc-macro span shapes an iterator-callback rule has to tell
// apart, in one crate so a single fixture can hold both.

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::{Group, Ident, Punct, Span, TokenStream, TokenTree};

/// An over-threshold callback in which every token wears one user
/// span, the way `quote_spanned!(field.span() => ...)` emits a whole
/// expansion at the span of something the caller wrote.
const SYNTHESISED: &str = "const _: () = { fn _synth() { \
                           let _ = [1, 2].into_iter().map(|value| { \
                           let doubled = value * 2; doubled }); } };";

/// Re-emits the `fn` keyword at `Span::call_site()` and passes the rest
/// of the item through, which is what `quote! { fn #name(#inputs)
/// #body }` produces: the signature is the macro's, the body is the
/// caller's own source.
#[proc_macro_attribute]
pub fn rebuilt_signature(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut trees = item.into_iter();
    let keyword = trees.next().expect("expected a `fn` keyword");
    assert!(
        matches!(&keyword, TokenTree::Ident(identifier) if identifier.to_string() == "fn"),
        "expected a `fn` keyword",
    );
    let mut rebuilt = TokenStream::new();
    rebuilt.extend([TokenTree::Ident(Ident::new("fn", Span::call_site()))]);
    rebuilt.extend(trees);
    rebuilt
}

/// The control: the item comes back exactly as the caller wrote it.
#[proc_macro_attribute]
pub fn verbatim(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Emits [`SYNTHESISED`] with every token re-spanned to the user span
/// of the `#[synth_callback]` attribute.
#[proc_macro_derive(SynthCallback, attributes(synth_callback))]
pub fn synth_callback(input: TokenStream) -> TokenStream {
    let span = find_attr_span(input, "synth_callback")
        .expect("`#[derive(SynthCallback)]` requires a `#[synth_callback]`");
    respan(
        SYNTHESISED.parse().expect("the synthesised source parses"),
        span,
    )
}

fn find_attr_span(input: TokenStream, name: &str) -> Option<Span> {
    for tree in input {
        match tree {
            TokenTree::Ident(identifier) if identifier.to_string() == name => {
                return Some(identifier.span());
            }
            TokenTree::Group(group) => {
                if let Some(span) = find_attr_span(group.stream(), name) {
                    return Some(span);
                }
            }
            _ => {}
        }
    }
    None
}

fn respan(stream: TokenStream, span: Span) -> TokenStream {
    stream
        .into_iter()
        .map(|tree| respan_tree(tree, span))
        .collect()
}

fn respan_tree(tree: TokenTree, span: Span) -> TokenTree {
    match tree {
        TokenTree::Group(group) => {
            let mut replacement = Group::new(group.delimiter(), respan(group.stream(), span));
            replacement.set_span(span);
            TokenTree::Group(replacement)
        }
        TokenTree::Ident(identifier) => TokenTree::Ident(Ident::new(&identifier.to_string(), span)),
        TokenTree::Punct(punct) => {
            let mut replacement = Punct::new(punct.as_char(), punct.spacing());
            replacement.set_span(span);
            TokenTree::Punct(replacement)
        }
        TokenTree::Literal(mut literal) => {
            literal.set_span(span);
            TokenTree::Literal(literal)
        }
    }
}

/// Re-emits the item unchanged and appends a synthesised iterator
/// callback respanned onto the span of a parenthesis group taken from
/// the caller's own tokens.
///
/// A group's span covers its delimiters, so a `(|value| ...)` group's
/// byte range is exactly the range a parenthesised callback is keyed
/// under. That makes this the one shape a stable proc macro can name
/// whose synthesised callback lands on a key the re-parse wrote. A
/// bare closure is four token trees, so no single input span equals
/// its range.
#[proc_macro_attribute]
pub fn respan_onto_paren(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let span = find_closure_paren(item.clone()).expect("expected a parenthesised closure");
    let mut out = TokenStream::new();
    out.extend(item);
    out.extend(respan(
        SYNTHESISED.parse().expect("the synthesised source parses"),
        span,
    ));
    out
}

fn find_closure_paren(stream: TokenStream) -> Option<Span> {
    for tree in stream {
        if let TokenTree::Group(group) = tree {
            let starts_with_pipe = matches!(
                group.stream().into_iter().next(),
                Some(TokenTree::Punct(ref punct)) if punct.as_char() == '|'
            );
            if group.delimiter() == proc_macro::Delimiter::Parenthesis && starts_with_pipe {
                return Some(group.span());
            }
            if let Some(span) = find_closure_paren(group.stream()) {
                return Some(span);
            }
        }
    }
    None
}
