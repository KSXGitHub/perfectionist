//! The index of every rule this plugin ships.
//!
//! `rule_index!` turns one list of rule names into what the rest of
//! the crate needs from it: a type per rule, the
//! [`LINT_NAMES`] set, and [`register_all`], which
//! [`crate::register_lints`] calls. Each rule's own module
//! implements [`Register`] for its type.
//!
//! The name set is what keeps the list free of ordering exceptions.
//! `unknown_perfectionist_lints` reports a `perfectionist::*` name
//! that this plugin does not ship, so it needs the whole set; read
//! back out of the `LintStore` that set is only complete once every
//! other rule has registered, which forced that one rule to the end
//! of the list. Read from [`LINT_NAMES`] it is complete before
//! registration starts, and the rule sits in the list alphabetically
//! like any other.

use crate::common::{DefaultState, resolved_state};
use rustc_lint::LintStore;

/// What the index needs from a rule, implemented by each rule module
/// for the type `rule_index!` generates for it.
pub(crate) trait Register {
    /// Whether the rule's pass installs when `dylint.toml` says
    /// nothing about the rule either way. Each rule's own impl
    /// documents why it chose what it chose; the choice itself is
    /// governed by the rule activation model in
    /// `planned-rules/IMPLEMENTATION_CONVENTIONS.md`. `gen-docs`
    /// reads the initializer with syn to render the rule's
    /// catalogue entry, so it has to be a plain [`DefaultState`]
    /// variant rather than anything computed.
    const DEFAULT_STATE: DefaultState;

    /// Add the rule's lint declaration to the store. Called for
    /// every rule, whatever `dylint.toml` says about it — the lint
    /// stays registered even where the pass does not install.
    fn register_lint(lint_store: &mut LintStore);

    /// Install the rule's pass. Called only for a rule that
    /// [`register_all`] resolved to [`DefaultState::Active`].
    fn register_pass(lint_store: &mut LintStore);
}

/// Expand the rule list into the index.
///
/// Each entry pairs the rule's name — the snake_case one it wears in
/// `dylint.toml` and in `#[allow(perfectionist::...)]`, and the name
/// of its `src/rules/<name>.rs` file — with the type to generate
/// for it. Both are spelled out because `macro_rules!`
/// cannot case-convert an identifier.
///
/// Entries are in ascending order, since [`is_registered_lint`]
/// binary-searches [`LINT_NAMES`]; the tests below hold them to it.
macro_rules! rule_index {
    ($( $rule_name:ident => $marker:ident ),+ $(,)?) => {
        /// One type per rule, standing in for it wherever the
        /// index needs something to implement [`Register`] on. They
        /// are a module of their own because a rule's type and the
        /// rule's lint pass want the same name.
        pub(crate) mod rule {
            $(
                #[doc = concat!(
                    "The `", stringify!($rule_name),
                    "` rule; `src/rules/", stringify!($rule_name),
                    ".rs` implements [`Register`](super::Register) for it.",
                )]
                pub(crate) struct $marker;
            )+
        }

        /// Every lint this plugin registers, unqualified — the name
        /// as it appears after the `perfectionist::` prefix — in
        /// ascending order.
        ///
        /// A sorted slice rather than a `LazyLock<HashSet>` or a
        /// `LazyLock<BTreeSet>`: at this size (a few dozen short
        /// names, laid out contiguously) a binary search over cached
        /// bytes beats hashing, and beats a tree's pointer chase.
        /// The set is also swept end to end whenever an unknown name
        /// is reported, to find the closest registered name to
        /// suggest — an order a slice already has and a `HashSet`
        /// does not. Being static, it costs no lazy initialisation
        /// and no allocation in the runs that never consult it.
        pub(crate) static LINT_NAMES: &[&str] = &[$( stringify!($rule_name) ),+];

        /// Register every rule: its lint declaration always, its
        /// pass where the rule resolves to active. Each rule is
        /// registered on its own — no entry depends on another
        /// having registered first.
        pub(crate) fn register_all(lint_store: &mut LintStore) {
            $(
                register_rule::<rule::$marker>(lint_store, stringify!($rule_name));
            )+
        }
    };
}

rule_index! {
    allow_attributes => AllowAttributes,
    allow_attributes_without_reason => AllowAttributesWithoutReason,
    arbitrary_source_item_ordering => ArbitrarySourceItemOrdering,
    avoidable_string_escapes => AvoidableStringEscapes,
    bare_email => BareEmail,
    bare_identifier_reference => BareIdentifierReference,
    bare_issue_reference => BareIssueReference,
    bare_url => BareUrl,
    clap_help_markdown => ClapHelpMarkdown,
    core_instead_of_std => CoreInsteadOfStd,
    excessive_cognitive_complexity => ExcessiveCognitiveComplexity,
    excessive_inline_tests => ExcessiveInlineTests,
    excessive_nesting => ExcessiveNesting,
    exhaustive_error_enums => ExhaustiveErrorEnums,
    import_granularity_mismatch => ImportGranularityMismatch,
    import_grouping_mismatch => ImportGroupingMismatch,
    impure_macro_arguments => ImpureMacroArguments,
    lint_attribute_trailing_comment => LintAttributeTrailingComment,
    macro_trailing_comma => MacroTrailingComma,
    named_prelude_imports => NamedPreludeImports,
    needless_borrowed_parameters => NeedlessBorrowedParameters,
    overly_long_file => OverlyLongFile,
    overly_long_function => OverlyLongFunction,
    overly_long_method_chain => OverlyLongMethodChain,
    overly_long_print_macro => OverlyLongPrintMacro,
    redundant_derive_more_forward_template => RedundantDeriveMoreForwardTemplate,
    single_letter_closure_param => SingleLetterClosureParam,
    single_letter_const_generic => SingleLetterConstGeneric,
    single_letter_const_item => SingleLetterConstItem,
    single_letter_function_param => SingleLetterFunctionParam,
    single_letter_generic => SingleLetterGeneric,
    single_letter_let_binding => SingleLetterLetBinding,
    single_letter_static_item => SingleLetterStaticItem,
    thiserror_usage => ThiserrorUsage,
    too_many_local_bindings => TooManyLocalBindings,
    uncombined_self_import => UncombinedSelfImport,
    unicode_ellipsis_in_comments => UnicodeEllipsisInComments,
    unicode_ellipsis_in_docs => UnicodeEllipsisInDocs,
    unicode_ellipsis_in_panic_messages => UnicodeEllipsisInPanicMessages,
    unknown_perfectionist_lints => UnknownPerfectionistLints,
    unordered_derives => UnorderedDerives,
    unpinned_repo_ref => UnpinnedRepoRef,
    wildcard_imports => WildcardImports,
}

/// Register one rule: its lint declaration always, its pass where
/// `dylint.toml` and [`Register::DEFAULT_STATE`] leave it active.
/// Module scope rather than the macro body, so each entry expands to
/// a call rather than to another copy of this.
fn register_rule<Rule: Register>(lint_store: &mut LintStore, name: &str) {
    Rule::register_lint(lint_store);
    match resolved_state(name, Rule::DEFAULT_STATE) {
        DefaultState::Active => Rule::register_pass(lint_store),
        DefaultState::Inactive => {}
    }
}

/// Whether `name` — with the `perfectionist::` prefix already
/// stripped — is a lint this plugin registers.
pub(crate) fn is_registered_lint(name: &str) -> bool {
    LINT_NAMES.binary_search(&name).is_ok()
}

#[cfg(test)]
mod tests;
