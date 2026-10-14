// force-host
// no-prefer-dynamic
//
// The proc-macro span shapes an iterator-callback rule has to tell
// apart, in one crate so a single fixture can hold them all.

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::{Delimiter, Group, Ident, Punct, Span, TokenStream, TokenTree};

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

/// Wraps the first closure in the item's body in a synthesised
/// `[1, 2].into_iter().map(...)`, passing the closure's own tokens
/// through untouched and leaving the synthesised tokens on the
/// `Span::call_site()` that `TokenStream::from_str` and `quote!` give
/// them.
///
/// This is the shape that needs no span trickery at all: the callback
/// keeps the caller's byte range, so it is keyed and
/// `Span::from_expansion` is false for it, while the call that selects
/// it exists only in the expansion. What rejects it is asking the same
/// question of the call's own span.
#[proc_macro_attribute]
pub fn wrap_in_iterator_call(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let (wrapped, found) = wrap_closure(item);
    assert!(found, "expected a closure in the item body");
    wrapped
}

fn wrap_closure(stream: TokenStream) -> (TokenStream, bool) {
    let trees: Vec<TokenTree> = stream.into_iter().collect();
    if let Some((start, end)) = closure_range(&trees) {
        let mut out = TokenStream::new();
        out.extend(trees[..start].iter().cloned());
        out.extend(
            "[1, 2].into_iter().map"
                .parse::<TokenStream>()
                .expect("the prefix parses"),
        );
        out.extend([TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            trees[start..end].iter().cloned().collect(),
        ))]);
        out.extend(trees[end..].iter().cloned());
        return (out, true);
    }
    let mut out = TokenStream::new();
    let mut found = false;
    for tree in trees {
        match tree {
            TokenTree::Group(group) if !found => {
                let (inner, hit) = wrap_closure(group.stream());
                found |= hit;
                let mut rebuilt = Group::new(group.delimiter(), inner);
                rebuilt.set_span(group.span());
                out.extend([TokenTree::Group(rebuilt)]);
            }
            other => out.extend([other]),
        }
    }
    (out, found)
}

/// The token-tree run of a closure written `|..| { .. }`, as a
/// half-open range into `trees`.
fn closure_range(trees: &[TokenTree]) -> Option<(usize, usize)> {
    let open = trees.iter().position(is_pipe)?;
    let close = trees[open + 1..].iter().position(is_pipe)? + open + 1;
    match trees.get(close + 1)? {
        TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => Some((open, close + 2)),
        _ => None,
    }
}

fn is_pipe(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Punct(punct) if punct.as_char() == '|')
}

/// A parenthesised two-statement callback, and the item a synthesised
/// call to it lives in. `__CALLBACK__` is spliced out for the callback
/// once the rest has been respanned, so the call and the callback end
/// up on two different spans of the caller's own.
const TWO_TARGET_CALLBACK: &str = "(|value| { let doubled = value * 2; doubled })";
const TWO_TARGET_CARRIER: &str = "const _: () = { fn _two_targets() { \
                                  let _ = [1, 2].into_iter().map(__CALLBACK__); } };";

/// Substitutes the ident `ITEMS` for an iterator, leaving the caller's
/// `.map(..)` and callback untouched.
///
/// A method call's span runs from its receiver to its closing
/// parenthesis, and combining spans of differing syntax context yields
/// the macro's. So this makes the *call* look macro-made while every
/// token of it but the receiver is the caller's. The callback here is
/// the caller's own and is reported.
#[proc_macro_attribute]
pub fn synthesise_receiver(_attr: TokenStream, item: TokenStream) -> TokenStream {
    substitute(item)
}

fn substitute(stream: TokenStream) -> TokenStream {
    let mut out = TokenStream::new();
    for tree in stream {
        match tree {
            TokenTree::Ident(ref identifier) if identifier.to_string() == "ITEMS" => out.extend(
                "[1, 2].into_iter()"
                    .parse::<TokenStream>()
                    .expect("the receiver parses"),
            ),
            TokenTree::Group(group) => {
                let mut rebuilt = Group::new(group.delimiter(), substitute(group.stream()));
                rebuilt.set_span(group.span());
                out.extend([TokenTree::Group(rebuilt)]);
            }
            other => out.extend([other]),
        }
    }
    out
}

/// Rebuilds every parenthesis group without carrying its span over,
/// and changes nothing else.
///
/// The milder version of `synthesise_receiver`: here *every* token is
/// the caller's and only the argument list's delimiters lost their
/// span, which is enough to make the call's combined span the macro's.
/// The callback is still reported.
#[proc_macro_attribute]
pub fn rebuild_argument_group(_attr: TokenStream, item: TokenStream) -> TokenStream {
    rebuild(item)
}

fn rebuild(stream: TokenStream) -> TokenStream {
    stream
        .into_iter()
        .map(|tree| match tree {
            TokenTree::Group(group) if group.delimiter() == Delimiter::Parenthesis => {
                TokenTree::Group(Group::new(Delimiter::Parenthesis, rebuild(group.stream())))
            }
            TokenTree::Group(group) => {
                let mut rebuilt = Group::new(group.delimiter(), rebuild(group.stream()));
                rebuilt.set_span(group.span());
                TokenTree::Group(rebuilt)
            }
            other => other,
        })
        .collect()
}

/// Appends a synthesised call whose tokens wear the item's first span
/// and whose synthesised callback wears a parenthesis group's, so the
/// two sit on separate spans the caller wrote.
///
/// Nothing here is the caller's code, so nothing is reported.
#[proc_macro_attribute]
pub fn respan_two_targets(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let early = item
        .clone()
        .into_iter()
        .next()
        .map(|tree| tree.span())
        .expect("expected at least one token");
    let paren = find_closure_paren(item.clone()).expect("expected a parenthesised closure");
    let callback = respan(
        TWO_TARGET_CALLBACK.parse().expect("the callback parses"),
        paren,
    );
    let carrier = respan(TWO_TARGET_CARRIER.parse().expect("the carrier parses"), early);
    let mut out = TokenStream::new();
    out.extend(item);
    out.extend(splice(carrier, &callback));
    out
}

fn splice(stream: TokenStream, replacement: &TokenStream) -> TokenStream {
    let mut out = TokenStream::new();
    for tree in stream {
        match tree {
            TokenTree::Ident(ref identifier) if identifier.to_string() == "__CALLBACK__" => {
                out.extend(replacement.clone());
            }
            TokenTree::Group(group) => {
                let mut rebuilt = Group::new(group.delimiter(), splice(group.stream(), replacement));
                rebuilt.set_span(group.span());
                out.extend([TokenTree::Group(rebuilt)]);
            }
            other => out.extend([other]),
        }
    }
    out
}

/// Substitutes the ident `helper` for an iterator method, so a call the
/// caller wrote to some other function resolves to `Iterator::map`.
///
/// This is why the recorded name is the one in the source rather than
/// the one the call resolves to. The caller's `helper(..)` is a call
/// with a closure as its final argument, so a record keyed on shape
/// alone would accept it; `helper` is not a method the rule covers, so
/// a record keyed on the written name does not. Nothing is reported.
#[proc_macro_attribute]
pub fn substitute_callee(_attr: TokenStream, item: TokenStream) -> TokenStream {
    rename(item)
}

fn rename(stream: TokenStream) -> TokenStream {
    let mut out = TokenStream::new();
    for tree in stream {
        match tree {
            TokenTree::Ident(ref identifier) if identifier.to_string() == "helper" => out.extend(
                "[1, 2].into_iter().map"
                    .parse::<TokenStream>()
                    .expect("the callee parses"),
            ),
            TokenTree::Group(group) => {
                let mut rebuilt = Group::new(group.delimiter(), rename(group.stream()));
                rebuilt.set_span(group.span());
                out.extend([TokenTree::Group(rebuilt)]);
            }
            other => out.extend([other]),
        }
    }
    out
}

