# Shared git-hook helper. Sourced, not executed — it defines a function
# and sets no `set -euo pipefail` of its own (the sourcing script owns
# that).

# Succeed (exit 0) iff the argument is a bare version literal: X.Y.Z or
# X.Y.Z-<suffix>, the shape a release carries. A commit whose subject is
# one is a version-bump commit, exempt from the subject-length check and
# the only kind the release contract applies to. A tag whose name is one
# is a release tag, and the contract applies to the commit it points at.
# The Rust side spells the same shape in
# tools/deploy-check/src/version_literal.rs.
# Args: <candidate>.
is_version_literal() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[^[:space:]]+)?$ ]]
}
