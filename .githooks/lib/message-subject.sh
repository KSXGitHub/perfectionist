# Shared git-hook helper. Sourced, not executed — it defines a function
# and sets no `set -euo pipefail` of its own (the sourcing script owns
# that).

# Echo the subject line of a git message file: the first non-blank,
# non-comment line. Args: <msg_file> <comment_char>.
message_subject() {
  awk -v cc="$2" 'index($0, cc) != 1 && NF { print; exit }' "$1"
}
