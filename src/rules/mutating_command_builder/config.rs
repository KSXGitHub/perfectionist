//! Configuration for `mutating_command_builder`: where `command-extra`
//! has to be declared before the lint will name its methods.

/// The user-facing configuration shape, deserialised from the
/// `["perfectionist::mutating_command_builder"]` table of
/// `dylint.toml`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct Config {
    /// How far to look for a declaration of `command-extra` before the
    /// lint will name its methods. Without the crate somewhere in
    /// reach, the diagnostic would name something the author cannot
    /// write. Defaults to `workspace`: a workspace that has settled on
    /// the crate has settled for its members, so a member that has not
    /// inherited it yet is told all the same — what it is missing is a
    /// line in a manifest. Narrow it to `crate` where the members are
    /// deliberately not uniform, or widen it to `unchecked` to hear
    /// from everywhere, including where the method named cannot be
    /// written yet.
    pub(super) command_extra_dependency: RequiredDeclaration,
}

/// Which manifest has to declare `command-extra`.
#[derive(Debug, Default, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RequiredDeclaration {
    /// The manifest of the crate under lint, whether it names a version
    /// of its own or inherits the workspace's with
    /// `command-extra.workspace = true`. A sibling that has not
    /// inherited it is left alone.
    Crate,
    /// That manifest, or the `[workspace.dependencies]` table of the
    /// workspace the crate belongs to, whether or not the crate has
    /// inherited the entry yet.
    #[default]
    Workspace,
    /// Neither: the lint speaks in every crate, including one where the
    /// method it names cannot be written until the dependency arrives.
    Unchecked,
}
