// ==========================================================================
// Whether a match is worth showing, decided by where it landed and
// reading no score.
// ==========================================================================

var perfectionistMatchAdmit = (function () {
  var isWordStart = perfectionistMatchText.isWordStart
  var isAlnum = perfectionistMatchText.isAlnum
  var better = perfectionistMatchScore.better

  // ---- Admission ----------------------------------------------------------

  // How many of a match's runs may begin inside a word. A reader typing a
  // name from memory opens words wherever they like — that is what finds
  // `bare_url` for `bur` — but a run beginning inside a word is a letter
  // that landed where it happened to occur rather than where the reader
  // aimed it. One of those is a slip of the fingers, and keeping it is
  // what still finds `excessive_nesting` for `excessive_nestng`. Two is
  // the characters falling where they may, which is how `bare` would
  // otherwise answer `needless_borrowed_parameters`.
  var SLIPS_ALLOWED = 1

  /**
   * Where the word holding `index` begins.
   * @param {string} haystack
   * @param {number} index
   * @returns {number}
   */
  function wordStartBefore(haystack, index) {
    var at = index
    while (at > 0 && isAlnum(haystack.charAt(at - 1))) {
      at--
    }
    return at
  }

  /**
   * Did the reader aim this run, or did it land where it happened to? It
   * is aimed when it opens a word, and also when it finishes one and
   * holds more of that word than it skipped: `error` is the back five of
   * `thiserror`'s nine, so a reader who types it has typed one of the
   * words that identifier ran together — where one who types the `e` that
   * `single` ends in has typed a letter that happens to fall last.
   * @param {string} haystack
   * @param {Span} range
   * @returns {boolean}
   */
  function aimed(haystack, range) {
    var start = range[0]
    if (isWordStart(haystack, start)) {
      return true
    }
    if (isAlnum(haystack.charAt(range[1]))) {
      return false
    }
    return range[1] - start >= start - wordStartBefore(haystack, start)
  }

  /**
   * Is this match worth showing at all?
   *
   * Nothing here reads the score, and that is the point: a score orders
   * matches against each other, where only the differences matter and the
   * numbers are free to be re-tuned, while this decides whether a reader
   * sees the target at all, where an answer that moves under re-tuning is
   * an answer nobody can rely on: one more character could push a result
   * back over a bar it had fallen under, so it leaves the list and
   * returns.
   * @param {readonly Span[]} ranges  the match's runs, in order, at least one
   * @param {string} haystack    folded target
   * @returns {boolean}
   */
  function admits(ranges, haystack) {
    if (!aimed(haystack, ranges[0])) {
      return false
    }
    var slips = 0
    for (var i = 1; i < ranges.length; i++) {
      if (isWordStart(haystack, ranges[i][0])) {
        continue
      }
      slips++
      if (slips > SLIPS_ALLOWED) {
        return false
      }
    }
    return true
  }

  /**
   * The better of two matches, counting one worth showing above one that
   * is not — between tiers and between two placements of one tier alike.
   * Within a tier that matters because an earlier coincidence would
   * otherwise cost a target its real match: `nic` scores the same inside
   * `unicode` as it does finishing `panic`, so choosing by score alone
   * kept the first and showed `unicode_ellipsis_in_panic_messages` for
   * neither.
   *
   * This only orders, so a tier whose every placement is a coincidence
   * still comes back for `matchFuzzy` to turn away.
   * @param {Match | null} left
   * @param {Match | null} right
   * @param {string} haystack
   * @returns {Match | null}
   */
  function betterAdmitted(left, right, haystack) {
    if (!left) {
      return right
    }
    if (!right) {
      return left
    }
    var leftShown = admits(left.ranges, haystack)
    if (leftShown !== admits(right.ranges, haystack)) {
      return leftShown ? left : right
    }
    return better(left, right)
  }

  return {
    SLIPS_ALLOWED: SLIPS_ALLOWED,
    wordStartBefore: wordStartBefore,
    aimed: aimed,
    admits: admits,
    betterAdmitted: betterAdmitted,
  }
})()
