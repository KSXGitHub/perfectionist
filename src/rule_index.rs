//! The index of every rule this plugin ships.
//!
//! `rule_index!` turns one list of rule names into the three
//! things the rest of the crate needs from it: a marker type per
//! rule, the [`LINT_NAMES`] set, and [`register_all`] — which
//! [`crate::register_lints`] calls, and which is the only place the
//! rule modules are reached from. Each rule's own module implements
//! [`RuleRegistration`] for its marker type.
//!
//! The name set is what keeps the list free of ordering exceptions.
//! `unknown_perfectionist_lints` reports a `perfectionist::*` name
//! that this plugin does not ship, so it needs the whole set; read
//! back out of the `LintStore` that set is only complete once every
//! other rule has registered, which forced that one rule to the end
//! of the list. Read from [`LINT_NAMES`] it is complete before
//! registration starts, and the rule sits in the list alphabetically
//! like any other.

use rustc_lint::LintStore;

/// What the index needs from a rule, implemented by each rule module
/// for the marker type `rule_index!` generates for it.
pub(crate) trait RuleRegistration {
    /// Add the rule's lint declaration to the store. Called for
    /// every rule, including one the user turned off: the lint stays
    /// registered either way so that
    /// `#[allow(perfectionist::<rule>)]` keeps resolving.
    fn register_lint(lint_store: &mut LintStore);

    /// Install the rule's pass, unless
    /// [`crate::common::resolved_state`] resolves the rule to
    /// [`crate::common::DefaultState::Inactive`].
    fn register_pass(lint_store: &mut LintStore);
}

/// Expand the rule list into the index.
///
/// Each entry pairs the rule's name — the snake_case one it wears in
/// `dylint.toml` and in `#[allow(perfectionist::...)]`, and the name
/// of its `src/rules/<name>.rs` file — with the marker type to
/// generate for it. Both are spelled out because `macro_rules!`
/// cannot case-convert an identifier.
///
/// Entries are in ascending order, which a `const` assertion below
/// enforces: [`is_registered_lint`] binary-searches [`LINT_NAMES`].
macro_rules! rule_index {
    ($( $rule_name:ident => $marker:ident ),+ $(,)?) => {
        $(
            #[doc = concat!(
                "Index marker for the `",
                stringify!($rule_name),
                "` rule.",
            )]
            pub(crate) struct $marker;
        )+

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

        /// Register every rule's lint and install its pass.
        pub(crate) fn register_all(lint_store: &mut LintStore) {
            $(
                <$marker as RuleRegistration>::register_lint(lint_store);
                <$marker as RuleRegistration>::register_pass(lint_store);
            )+
        }
    };
}

rule_index! {
    allow_attributes => AllowAttributesRule,
    allow_attributes_without_reason => AllowAttributesWithoutReasonRule,
    avoidable_string_escapes => AvoidableStringEscapesRule,
    bare_email => BareEmailRule,
    bare_identifier_reference => BareIdentifierReferenceRule,
    bare_issue_reference => BareIssueReferenceRule,
    bare_url => BareUrlRule,
    clap_help_markdown => ClapHelpMarkdownRule,
    excessive_inline_tests => ExcessiveInlineTestsRule,
    exhaustive_error_enums => ExhaustiveErrorEnumsRule,
    import_granularity_mismatch => ImportGranularityMismatchRule,
    import_grouping_mismatch => ImportGroupingMismatchRule,
    impure_macro_arguments => ImpureMacroArgumentsRule,
    lint_attribute_trailing_comment => LintAttributeTrailingCommentRule,
    macro_trailing_comma => MacroTrailingCommaRule,
    named_prelude_imports => NamedPreludeImportsRule,
    needless_borrowed_parameters => NeedlessBorrowedParametersRule,
    overly_long_print_macro => OverlyLongPrintMacroRule,
    redundant_derive_more_forward_template => RedundantDeriveMoreForwardTemplateRule,
    single_letter_closure_param => SingleLetterClosureParamRule,
    single_letter_const_generic => SingleLetterConstGenericRule,
    single_letter_const_item => SingleLetterConstItemRule,
    single_letter_function_param => SingleLetterFunctionParamRule,
    single_letter_generic => SingleLetterGenericRule,
    single_letter_let_binding => SingleLetterLetBindingRule,
    single_letter_static_item => SingleLetterStaticItemRule,
    thiserror_usage => ThiserrorUsageRule,
    uncombined_self_import => UncombinedSelfImportRule,
    unicode_ellipsis_in_comments => UnicodeEllipsisInCommentsRule,
    unicode_ellipsis_in_docs => UnicodeEllipsisInDocsRule,
    unicode_ellipsis_in_panic_messages => UnicodeEllipsisInPanicMessagesRule,
    unknown_perfectionist_lints => UnknownPerfectionistLintsRule,
    unordered_derives => UnorderedDerivesRule,
    unpinned_repo_ref => UnpinnedRepoRefRule,
    wildcard_imports => WildcardImportsRule,
}

const _: () = assert!(
    is_ascending(LINT_NAMES),
    "rule_index! entries must be in ascending order",
);

/// Whether `name` is one of [`LINT_NAMES`].
pub(crate) fn is_registered_lint(name: &str) -> bool {
    LINT_NAMES.binary_search(&name).is_ok()
}

/// Whether `names` is strictly ascending, and so a valid subject for
/// [`slice::binary_search`].
const fn is_ascending(names: &[&str]) -> bool {
    let mut index = 1;
    while index < names.len() {
        if !precedes(names[index - 1].as_bytes(), names[index].as_bytes()) {
            return false;
        }
        index += 1;
    }
    true
}

/// Whether `left` sorts before `right`, comparing bytes — the order
/// [`str`]'s own comparison operators define, spelled out here
/// because they are not `const`.
const fn precedes(left: &[u8], right: &[u8]) -> bool {
    let mut index = 0;
    while index < left.len() && index < right.len() {
        if left[index] != right[index] {
            return left[index] < right[index];
        }
        index += 1;
    }
    left.len() < right.len()
}

#[cfg(test)]
mod tests;
