# Shared helpers for the `commit-msg` checks in `.githooks/commit-msg.d/`.
# This file is sourced, not executed — it defines functions and sets no
# `set -euo pipefail` of its own (the sourcing script owns that).

# Echo the effective comment char. Honour `core.commentChar` (default
# `#`) so a user with a non-default comment char doesn't have template
# comment lines leak through — that would otherwise let a malformed
# release commit slip past the subject checks, and make the bare-ref
# scan skip lines it should read. Fall back to `#` on unset, `auto`, or
# any non-single-char value, matching the Rust side's fallback.
cm_comment_char() {
  local cc
  cc=$(git config --get core.commentChar 2>/dev/null || true)
  case "$cc" in
    '') cc='#' ;;
    ?) ;;
    *) cc='#' ;;
  esac
  printf '%s' "$cc"
}

# Print the human-authored body of the message: drop comment-char lines
# and stop at the `git commit -v` scissors marker (a comment line
# carrying `>8`). Args: <msg_file> <comment_char>.
cm_human_lines() {
  awk -v cc="$2" '
    index($0, cc) == 1 { if (index($0, ">8") > 0) exit; next }
    { print }
  ' "$1"
}

# Echo the subject line: the first non-blank, non-comment line.
# Args: <msg_file> <comment_char>.
cm_subject() {
  awk -v cc="$2" 'index($0, cc) != 1 && NF { print; exit }' "$1"
}

# Succeed (exit 0) iff the given subject is a bare version literal
# (X.Y.Z or X.Y.Z-<suffix>) — the shape a version-bump commit uses.
# Such commits are exempt from the subject-length check and are the
# only ones the release contract applies to. Args: <subject>.
cm_is_version_subject() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[^[:space:]]+)?$ ]]
}
