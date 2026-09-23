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
/// `fn _synth_closure_body() { let _ = (|x| { let _ = 1; x })(0u32); }`
/// where the closure parameter `x` inherits the user-span of
/// `synth_closure`. The body is non-trivial so the
/// single-letter-closure-param rule would fire on hand-written
/// equivalent code; the trailing `(0u32)` call pins the closure
/// parameter's type so the function typechecks.
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

/// `#[derive(SynthConstItem)]` + `#[synth_const_item]` →
/// `const _: () = { const X: u32 = 1; let _ = X; };` where the
/// inner `X` const identifier inherits the user-span of
/// `synth_const_item`. Exercises the single-letter-const-item
/// rule's `hir_in_external_macro` guard.
#[proc_macro_derive(SynthConstItem, attributes(synth_const_item))]
pub fn synth_const_item(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_const_item")
        .expect("`#[derive(SynthConstItem)]` requires a `#[synth_const_item]`");
    let x_ident = Ident::new("X", attr_span);
    let call_site = Span::call_site();

    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("const", call_site)),
        TokenTree::Ident(x_ident.clone()),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("u32", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Literal(Literal::u32_unsuffixed(1)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Ident(x_ident),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);
    wrap_const_block(body)
}

/// `#[derive(SynthStaticItem)]` + `#[synth_static_item]` →
/// `fn _synth_static_item_body() { static X: u32 = 1; let _ = X; }`
/// where the inner `X` static identifier inherits the user-span of
/// `synth_static_item`. Exercises the single-letter-static-item
/// rule's `hir_in_external_macro` guard.
#[proc_macro_derive(SynthStaticItem, attributes(synth_static_item))]
pub fn synth_static_item(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_static_item")
        .expect("`#[derive(SynthStaticItem)]` requires a `#[synth_static_item]`");
    let x_ident = Ident::new("X", attr_span);
    let call_site = Span::call_site();

    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("static", call_site)),
        TokenTree::Ident(x_ident.clone()),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("u32", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Literal(Literal::u32_unsuffixed(1)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Ident(x_ident),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);
    wrap_fn_block("_synth_static_item_body", body)
}

/// `#[derive(SynthConstGeneric)]` + `#[synth_const_generic]` →
/// `const _: () = { fn _synth<const X: usize>() { let _ = X; } };`
/// where the `X` const generic parameter inherits the user-span of
/// `synth_const_generic`. Exercises the single-letter-const-generic
/// rule's `hir_in_external_macro` guard.
#[proc_macro_derive(SynthConstGeneric, attributes(synth_const_generic))]
pub fn synth_const_generic(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_const_generic")
        .expect("`#[derive(SynthConstGeneric)]` requires a `#[synth_const_generic]`");
    let x_ident = Ident::new("X", attr_span);
    let call_site = Span::call_site();

    let mut generics = TokenStream::new();
    generics.extend([
        TokenTree::Punct(Punct::new('<', Spacing::Alone)),
        TokenTree::Ident(Ident::new("const", call_site)),
        TokenTree::Ident(x_ident.clone()),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("usize", call_site)),
        TokenTree::Punct(Punct::new('>', Spacing::Alone)),
    ]);
    let mut fn_body = TokenStream::new();
    fn_body.extend([
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Ident(x_ident),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);
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

/// `#[derive(SynthFieldRefBody)]` → emits
/// `const _: () = { let _ = (); };` whose `let _ = ();` statement is
/// spanned over the derived type's body `{ ... }` — the span that
/// covers its field / variant doc comments. The `const _` wrapper is
/// anonymous, so the derive can be applied to several types in one
/// crate without name collisions.
///
/// This mimics how a real proc-macro derive such as
/// `serde::Deserialize` lays out its generated `visit_map` / `visit_seq`
/// bodies: a root-context HIR node whose span spans the whole field
/// list, lowered *after* the struct itself. The comment-anchor finder
/// must keep the documented field/variant as the anchor (the tightest
/// containing node) rather than letting this wider, later-visited
/// generated node steal it — the bug reported in issue #165's
/// follow-up. Built-in derives (`Debug`, `Default`, ...) don't produce
/// such a node, which is why they don't reproduce it.
#[proc_macro_derive(SynthFieldRefBody)]
pub fn synth_field_ref_body(input: TokenStream) -> TokenStream {
    // The body `{ ... }` group span covers the field / variant doc
    // comments inside it.
    let body_span = input
        .into_iter()
        .find_map(|tree| match tree {
            TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => Some(group.span()),
            _ => None,
        })
        .unwrap_or_else(Span::call_site);
    let at_body = |mut tree: TokenTree| {
        tree.set_span(body_span);
        tree
    };
    let mut body = TokenStream::new();
    body.extend([
        at_body(TokenTree::Ident(Ident::new("let", body_span))),
        at_body(TokenTree::Ident(Ident::new("_", body_span))),
        at_body(TokenTree::Punct(Punct::new('=', Spacing::Alone))),
        at_body(TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            TokenStream::new(),
        ))),
        at_body(TokenTree::Punct(Punct::new(';', Spacing::Alone))),
    ]);
    wrap_const_block(body)
}

/// `#[derive(SynthSilenceReason)]` + `#[synth_silence_reason]` →
/// `#[allow(dead_code)] const _: () = ();` whose generated `#[allow]`
/// inherits the user-span of `synth_silence_reason`, mirroring the
/// `#[allow(...)]` shape `clap_derive` emits (issue #430). The anchor is
/// an anonymous `const _` so the derive can be applied to several types
/// in one crate without colliding.
#[proc_macro_derive(SynthSilenceReason, attributes(synth_silence_reason))]
pub fn synth_silence_reason(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_silence_reason")
        .expect("`#[derive(SynthSilenceReason)]` requires a `#[synth_silence_reason]`");
    let call_site = Span::call_site();

    // `allow(dead_code)`, every token stamped with the user span so the
    // generated attribute looks (to span-based filters) like it was
    // hand-written at the `#[synth_silence_reason]` site.
    let at_attr = |mut tree: TokenTree| {
        tree.set_span(attr_span);
        tree
    };
    let mut allow_args = TokenStream::new();
    allow_args.extend([at_attr(TokenTree::Ident(Ident::new("dead_code", attr_span)))]);
    let mut attr_inner = TokenStream::new();
    attr_inner.extend([
        at_attr(TokenTree::Ident(Ident::new("allow", attr_span))),
        at_attr(TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            allow_args,
        ))),
    ]);

    let mut out = TokenStream::new();
    out.extend([
        at_attr(TokenTree::Punct(Punct::new('#', Spacing::Alone))),
        at_attr(TokenTree::Group(Group::new(Delimiter::Bracket, attr_inner))),
        TokenTree::Ident(Ident::new("const", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);
    out
}

/// `#[derive(SynthAllowRewriteable)]` + `#[synth_allow_rewriteable]` →
/// `#[allow(non_snake_case)] const _: () = ();` whose generated `#[allow]`
/// inherits the user-span of `synth_allow_rewriteable`. Unlike
/// `SynthSilenceReason` (which emits the exempt `dead_code`), this names a
/// rewriteable built-in lint, so it exercises `allow_attributes`'s
/// proc-macro guard (issue #430): without the guard the rule would rewrite
/// a suppression the user never wrote.
#[proc_macro_derive(SynthAllowRewriteable, attributes(synth_allow_rewriteable))]
pub fn synth_allow_rewriteable(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_allow_rewriteable")
        .expect("`#[derive(SynthAllowRewriteable)]` requires a `#[synth_allow_rewriteable]`");
    let call_site = Span::call_site();

    // `allow(non_snake_case)`, every token stamped with the user span so the
    // generated attribute looks (to span-based filters) like it was
    // hand-written at the `#[synth_allow_rewriteable]` site.
    let at_attr = |mut tree: TokenTree| {
        tree.set_span(attr_span);
        tree
    };
    let mut allow_args = TokenStream::new();
    allow_args.extend([at_attr(TokenTree::Ident(Ident::new(
        "non_snake_case",
        attr_span,
    )))]);
    let mut attr_inner = TokenStream::new();
    attr_inner.extend([
        at_attr(TokenTree::Ident(Ident::new("allow", attr_span))),
        at_attr(TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            allow_args,
        ))),
    ]);

    let mut out = TokenStream::new();
    out.extend([
        at_attr(TokenTree::Punct(Punct::new('#', Spacing::Alone))),
        at_attr(TokenTree::Group(Group::new(Delimiter::Bracket, attr_inner))),
        TokenTree::Ident(Ident::new("const", call_site)),
        TokenTree::Ident(Ident::new("_", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);
    out
}

/// `#[derive(SynthOwnedParam)]` + `#[synth_owned_param]` →
/// `const _: () = { fn _synth(item: &str) -> String { item.to_owned() } };`
/// where the `&str` parameter *type* inherits the user-span of
/// `synth_owned_param`. `needless_borrowed_parameters` reports at the
/// parameter-type span, so a span-only filter sees a user-written
/// `&str` and would rewrite a signature the user never wrote; this
/// exercises the rule's `hir_in_external_macro` guard.
#[proc_macro_derive(SynthOwnedParam, attributes(synth_owned_param))]
pub fn synth_owned_param(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_owned_param")
        .expect("`#[derive(SynthOwnedParam)]` requires a `#[synth_owned_param]`");
    let call_site = Span::call_site();
    let at_type = |mut tree: TokenTree| {
        tree.set_span(attr_span);
        tree
    };

    // `item: &str` — only the `&str` type tokens carry the user span.
    let mut params = TokenStream::new();
    params.extend([
        TokenTree::Ident(Ident::new("item", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        at_type(TokenTree::Punct(Punct::new('&', Spacing::Alone))),
        at_type(TokenTree::Ident(Ident::new("str", attr_span))),
    ]);

    // `item.to_owned()`
    let mut fn_body = TokenStream::new();
    fn_body.extend([
        TokenTree::Ident(Ident::new("item", call_site)),
        TokenTree::Punct(Punct::new('.', Spacing::Alone)),
        TokenTree::Ident(Ident::new("to_owned", call_site)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
    ]);

    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("fn", call_site)),
        TokenTree::Ident(Ident::new("_synth", call_site)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, params)),
        TokenTree::Punct(Punct::new('-', Spacing::Joint)),
        TokenTree::Punct(Punct::new('>', Spacing::Alone)),
        TokenTree::Ident(Ident::new("String", call_site)),
        TokenTree::Group(Group::new(Delimiter::Brace, fn_body)),
    ]);
    wrap_const_block(body)
}

/// `#[derive(SynthCloningGetter)]` + `#[synth_cloning_getter]` →
/// `const _: () = { struct _Synth { s: String } impl _Synth {
/// fn s(&self) -> String { self.s.clone() } } };` where the
/// generated `impl` and every token of the method inside it inherit
/// the user-span of `synth_cloning_getter`, the way a `getset`-style
/// derive spans an accessor over the field it accesses. `cloning_getter`
/// reports at the method's `def_span`, so that user span defeats
/// `Span::from_expansion` and `report_in_external_macro: false`, and
/// stamping the `impl` too leaves a parent-item span check nothing to
/// find; this exercises the rule's `is_from_proc_macro` guard.
#[proc_macro_derive(SynthCloningGetter, attributes(synth_cloning_getter))]
pub fn synth_cloning_getter(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_cloning_getter")
        .expect("`#[derive(SynthCloningGetter)]` requires a `#[synth_cloning_getter]`");
    let call_site = Span::call_site();
    let at_sig = |mut tree: TokenTree| {
        tree.set_span(attr_span);
        tree
    };

    // `struct _Synth { s: String }` — entirely at the call site.
    let mut field = TokenStream::new();
    field.extend([
        TokenTree::Ident(Ident::new("s", call_site)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("String", call_site)),
    ]);
    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("struct", call_site)),
        TokenTree::Ident(Ident::new("_Synth", call_site)),
        TokenTree::Group(Group::new(Delimiter::Brace, field)),
    ]);

    // `self.s.clone()` — stamped like the signature, the way
    // `quote_spanned!(field.span() => ...)` stamps a whole accessor.
    let mut fn_body = TokenStream::new();
    fn_body.extend([
        at_sig(TokenTree::Ident(Ident::new("self", attr_span))),
        at_sig(TokenTree::Punct(Punct::new('.', Spacing::Alone))),
        at_sig(TokenTree::Ident(Ident::new("s", attr_span))),
        at_sig(TokenTree::Punct(Punct::new('.', Spacing::Alone))),
        at_sig(TokenTree::Ident(Ident::new("clone", attr_span))),
        at_sig(TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            TokenStream::new(),
        ))),
    ]);

    // `fn s(&self) -> String` — every signature token user-spanned. The
    // method is named for the field it reads, so `cloning_getter`'s
    // field-match clause admits it and only the proc-macro guard stops
    // the diagnostic; a method named anything else would leave the
    // fixture passing with the guard removed.
    let mut receiver = TokenStream::new();
    receiver.extend([
        at_sig(TokenTree::Punct(Punct::new('&', Spacing::Alone))),
        at_sig(TokenTree::Ident(Ident::new("self", attr_span))),
    ]);
    let mut method = TokenStream::new();
    method.extend([
        at_sig(TokenTree::Ident(Ident::new("fn", attr_span))),
        at_sig(TokenTree::Ident(Ident::new("s", attr_span))),
        at_sig(TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            receiver,
        ))),
        at_sig(TokenTree::Punct(Punct::new('-', Spacing::Joint))),
        at_sig(TokenTree::Punct(Punct::new('>', Spacing::Alone))),
        at_sig(TokenTree::Ident(Ident::new("String", attr_span))),
        at_sig(TokenTree::Group(Group::new(Delimiter::Brace, fn_body))),
    ]);

    // `impl _Synth { ... }` — stamped as well, so that every span the
    // rule could consult, the method's and its parent item's alike, reads
    // as user-written. A span-based guard has nothing left to go on.
    body.extend([
        at_sig(TokenTree::Ident(Ident::new("impl", attr_span))),
        at_sig(TokenTree::Ident(Ident::new("_Synth", attr_span))),
        at_sig(TokenTree::Group(Group::new(Delimiter::Brace, method))),
    ]);
    wrap_const_block(body)
}

/// `#[derive(SynthCommandSetter)]` + `#[synth_command_setter]` →
/// `fn _synth_command_setter() { let mut command =`
/// `std::process::Command::new("ls"); command.arg("-l"); }` where the
/// whole setter call, the `arg` segment included, inherits the
/// user-span of `synth_command_setter`.
///
/// The wrapping `fn` is stamped too, the way `SynthCloningGetter` stamps
/// its `impl`, so every span the rule could consult reads as
/// user-written and `hir_in_external_macro` -- which checks the node's
/// span and the enclosing item's `def_span` -- has nothing to find. What
/// stops the diagnostic is `is_from_proc_macro`, which reads the source
/// text under the span instead.
///
/// The synthesised call is one the rule fires on when hand-written --
/// a std setter on an owned local -- so the fixture is not vacuous.
#[proc_macro_derive(SynthCommandSetter, attributes(synth_command_setter))]
pub fn synth_command_setter(input: TokenStream) -> TokenStream {
    let attr_span = find_attr_span(input, "synth_command_setter")
        .expect("`#[derive(SynthCommandSetter)]` requires a `#[synth_command_setter]`");
    let call_site = Span::call_site();
    let at_attr = |mut tree: TokenTree| {
        tree.set_span(attr_span);
        tree
    };
    let path = |segments: &[&str], span: Span| {
        let mut out = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            if index > 0 {
                out.push(TokenTree::Punct(Punct::new(':', Spacing::Joint)));
                out.push(TokenTree::Punct(Punct::new(':', Spacing::Alone)));
            }
            out.push(TokenTree::Ident(Ident::new(segment, span)));
        }
        out
    };

    // `let mut command = std::process::Command::new("ls");` — at the
    // call site, so only the setter call below carries a user span.
    let mut new_args = TokenStream::new();
    new_args.extend([TokenTree::Literal(Literal::string("ls"))]);
    let mut body = TokenStream::new();
    body.extend([
        TokenTree::Ident(Ident::new("let", call_site)),
        TokenTree::Ident(Ident::new("mut", call_site)),
        TokenTree::Ident(Ident::new("command", call_site)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
    ]);
    body.extend(path(&["std", "process", "Command", "new"], call_site));
    body.extend([
        TokenTree::Group(Group::new(Delimiter::Parenthesis, new_args)),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]);

    // `command.arg("-l");` — every token user-spanned, the way a derive
    // spans a synthesised call over the attribute that drove it.
    let mut arg_args = TokenStream::new();
    arg_args.extend([TokenTree::Literal(Literal::string("-l"))]);
    body.extend([
        at_attr(TokenTree::Ident(Ident::new("command", attr_span))),
        at_attr(TokenTree::Punct(Punct::new('.', Spacing::Alone))),
        at_attr(TokenTree::Ident(Ident::new("arg", attr_span))),
        at_attr(TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            arg_args,
        ))),
        at_attr(TokenTree::Punct(Punct::new(';', Spacing::Alone))),
    ]);
    // `wrap_fn_block` would leave the `fn` at the call site; stamp it
    // so no span the rule reads betrays the expansion.
    let mut out = TokenStream::new();
    out.extend([
        at_attr(TokenTree::Ident(Ident::new("fn", attr_span))),
        at_attr(TokenTree::Ident(Ident::new("_synth_command_setter", attr_span))),
        at_attr(TokenTree::Group(Group::new(
            Delimiter::Parenthesis,
            TokenStream::new(),
        ))),
        at_attr(TokenTree::Group(Group::new(Delimiter::Brace, body))),
    ]);
    out
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
