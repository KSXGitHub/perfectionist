# Shared git-hook helper. Sourced, not executed — it defines a function
# and sets no `set -euo pipefail` of its own (the sourcing script owns
# that).

# Print the human-authored body of a git message file: drop comment-char
# lines and stop at the `git commit -v` scissors marker (a comment line
# carrying `>8`). Args: <msg_file> <comment_char>.
message_body() {
  awk -v cc="$2" '
    index($0, cc) == 1 { if (index($0, ">8") > 0) exit; next }
    { print }
  ' "$1"
}
