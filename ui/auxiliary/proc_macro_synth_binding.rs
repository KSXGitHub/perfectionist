// force-host
// no-prefer-dynamic
//
// A miniature stand-in for `clap_derive`'s `default_value_t` expansion.
// Each derive emits a synthesised node whose key identifier carries a
// *user* span (the span of an attribute identifier the user wrote)
// rather than a call-site span. That span pattern is what makes
// proc-macro derives slip past rustc's built-in
// `report_in_external_macro: false` check on rules whose diagnostic
// span is the identifier itself.
//
// The derives are split per node shape so that each `*_proc_macro.rs`
// regression fixture exercises one rule independently.

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

/// `#[derive(SynthBinding)]` + `#[synth_default = ..]` →
/// `const _: () = { let s = 1; let _ = s; };` where `s` inherits
/// the user-span of `synth_default`. Mirrors `clap_derive`'s
/// `default_value_t` expansion shape.
#[proc_macro_derive(SynthBinding, attributes(synth_default))]
pub fn synth_binding(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_default")
        .expect("`#[derive(SynthBinding)]` requires a `#[synth_default = ..]`");
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
    wrap_const_block(body)
}

/// `#[derive(SynthFnParam)]` + `#[synth_param]` →
/// `const _: () = { fn _synth(x: u32) {} };` where the `x` parameter
/// inherits the user-span of `synth_param`.
#[proc_macro_derive(SynthFnParam, attributes(synth_param))]
pub fn synth_fn_param(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_param")
        .expect("`#[derive(SynthFnParam)]` requires a `#[synth_param]`");
    let x_ident = Ident::new("x", attr_span);
    let call_site = Span::call_site();

    let mut params = TokenStream::new();
    params.extend([
        TokenTree::Ident(x_ident),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("u32", call_site)),
    ]);
    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("fn", call_site)),
        TokenTree::Ident(Ident::new("_synth", call_site)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, params)),
        TokenTree::Group(Group::new(Delimiter::Brace, TokenStream::new())),
    ]);
    wrap_const_block(body)
}

/// `#[derive(SynthGeneric)]` + `#[synth_generic]` →
/// `const _: () = { fn _synth<T>() { let _ = std::marker::PhantomData::<T>; } };`
/// where the `T` generic parameter inherits the user-span of
/// `synth_generic`.
#[proc_macro_derive(SynthGeneric, attributes(synth_generic))]
pub fn synth_generic(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_generic")
        .expect("`#[derive(SynthGeneric)]` requires a `#[synth_generic]`");
    let t_ident = Ident::new("T", attr_span);
    let call_site = Span::call_site();

    let mut generics = TokenStream::new();
    generics.extend([
        TokenTree::Punct(Punct::new('<', Spacing::Alone)),
        TokenTree::Ident(t_ident.clone()),
        TokenTree::Punct(Punct::new('>', Spacing::Alone)),
    ]);
    let mut phantom_path = TokenStream::new();
    phantom_path.extend([
        TokenTree::Ident(Ident::new("std", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("marker", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("PhantomData", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Punct(Punct::new('<', Spacing::Alone)),
        TokenTree::Ident(t_ident),
        TokenTree::Punct(Punct::new('>', Spacing::Alone)),
    ]);
    let mut fn_body = TokenStream::new();
    fn_body.extend([
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
    ]);
    fn_body.extend(phantom_path);
    fn_body.extend([TokenTree::Punct(Punct::new(';', Spacing::Alone))]);

    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("fn", call_site)),
        TokenTree::Ident(Ident::new("_synth", call_site)),
    ]);
    body.extend(generics);
    body.extend([
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Group(Group::new(Delimiter::Brace, fn_body)),
    ]);
    wrap_const_block(body)
}

/// `#[derive(SynthClosure)]` + `#[synth_closure]` →
/// `const _: () = { let _ = |x| { let _ = 1; x }; };` where the
/// closure parameter `x` inherits the user-span of `synth_closure`.
/// The body is non-trivial so the single-letter-closure-param rule
/// would fire on hand-written equivalent code.
#[proc_macro_derive(SynthClosure, attributes(synth_closure))]
pub fn synth_closure(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_closure")
        .expect("`#[derive(SynthClosure)]` requires a `#[synth_closure]`");
    let x_param = Ident::new("x", attr_span);
    let call_site = Span::call_site();

    let mut closure_body = TokenStream::new();
    closure_body.extend([
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Literal(Literal::u32_unsuffixed(1)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
        TokenTree::Ident(x_param.clone()),
    ]);
    let mut call_args = TokenStream::new();
    call_args.extend([TokenTree::Literal(Literal::u32_suffixed(0))]);
    let mut closure_tokens = TokenStream::new();
    closure_tokens.extend([
        TokenTree::Punct(Punct::new('|', Spacing::Alone)),
        TokenTree::Ident(x_param),
        TokenTree::Punct(Punct::new('|', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Brace, closure_body)),
    ]);
    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, closure_tokens)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, call_args)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);
    wrap_fn_block("_synth_closure_body", body)
}

/// `#[derive(SynthArcClone)]` + `#[synth_arc]` →
/// `const _: () = { let arc = std::sync::Arc::new(1u32); let _ = arc.clone(); };`
/// where the `.clone()` method-call segment inherits the user-span
/// of `synth_arc`. Exercises the method-call shape the
/// `arc_rc_clone` rule fires on.
#[proc_macro_derive(SynthArcClone, attributes(synth_arc))]
pub fn synth_arc_clone(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_arc")
        .expect("`#[derive(SynthArcClone)]` requires a `#[synth_arc]`");
    let clone_ident = Ident::new("clone", attr_span);
    let call_site = Span::call_site();

    let mut new_args = TokenStream::new();
    new_args.extend([TokenTree::Literal(Literal::u32_suffixed(1))]);
    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("arc", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Ident(Ident::new("std", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("sync", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("Arc", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("new", call_site)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, new_args)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Ident(Ident::new("arc", call_site)),
        TokenTree::Punct(Punct::new('.', Spacing::Alone)),
        TokenTree::Ident(clone_ident),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);
    wrap_fn_block("_synth_arc_clone_body", body)
}

fn wrap_const_block(body: TokenStream) -> TokenStream {
    let call_site = Span::call_site();
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

/// Wrap a body in `fn <fn_name>() { ... }` rather than `const _: () = { ... };`
/// for derives whose synthesised body cannot be evaluated at compile
/// time (closure calls, `Arc::new`, etc.). Callers pass a distinct
/// `fn_name` per derive so that two `wrap_fn_block`-using derives can
/// be applied to the same crate without colliding. The function is
/// unused so the body is never actually executed at runtime; rustc
/// still typechecks it and the late lint pass still walks the HIR.
fn wrap_fn_block(fn_name: &str, body: TokenStream) -> TokenStream {
    let call_site = Span::call_site();
    let mut out = TokenStream::new();
    out.extend([
        TokenTree::Ident(Ident::new("fn", call_site)),
        TokenTree::Ident(Ident::new(fn_name, call_site)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Group(Group::new(Delimiter::Brace, body)),
    ]);
    out
}

fn find_attr_span(input: TokenStream, name: &str) -> Option<Span> {
    let tokens: Vec<TokenTree> = input.into_iter().collect();
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
                if ident.to_string() == name {
                    return Some(ident.span());
                }
            }
        }
    }
    None
}
