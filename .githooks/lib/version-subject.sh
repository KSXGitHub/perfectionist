# Shared git-hook helper. Sourced, not executed — it defines a function
# and sets no `set -euo pipefail` of its own (the sourcing script owns
# that).

# Succeed (exit 0) iff the given subject is a bare version literal
# (X.Y.Z or X.Y.Z-<suffix>) — the shape a version-bump commit uses.
# Such commits are exempt from the subject-length check and are the
# only ones the release contract applies to. Args: <subject>.
is_version_subject() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[^[:space:]]+)?$ ]]
}
