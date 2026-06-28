# Shared git-hook helper. Sourced, not executed — it defines a function
# and sets no `set -euo pipefail` of its own (the sourcing script owns
# that).

# Echo the effective comment char. Honour `core.commentChar` (default
# `#`) so a user with a non-default comment char doesn't have template
# comment lines leak through — that would otherwise let a malformed
# release commit slip past the subject checks, and make the bare-ref
# scan skip lines it should read. Fall back to `#` on unset, `auto`, or
# any non-single-char value, matching the Rust side's fallback.
git_comment_char() {
  local cc
  cc=$(git config --get core.commentChar 2>/dev/null || true)
  case "$cc" in
    '') cc='#' ;;
    ?) ;;
    *) cc='#' ;;
  esac
  printf '%s' "$cc"
}
