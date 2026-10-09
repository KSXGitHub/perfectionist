// ==========================================================================
// Query matching: scoring a query against a string, and narrowing a long
// string to the part that matched. Nothing here touches the DOM and
// nothing here knows what a lint is, which is what lets it run with no
// browser around it.
// ==========================================================================

var perfectionistMatch = (function () {
  var fold = perfectionistMatchText.fold
  var admits = perfectionistMatchAdmit.admits
  var betterAdmitted = perfectionistMatchAdmit.betterAdmitted
  var matchVerbatim = perfectionistMatchTiers.matchVerbatim
  var matchRespaced = perfectionistMatchTiers.matchRespaced
  var matchVariants = perfectionistMatchTiers.matchVariants
  var matchReordered = perfectionistMatchTiers.matchReordered
  var matchScattered = perfectionistMatchTiers.matchScattered

  /**
   * Score `query` against `target`, allowing the query's characters to be
   * scattered through it as long as they occur in order. Returns the score
   * together with the half-open `[start, end)` character ranges of
   * `target` that matched, or `null` when they don't all occur in order.
   * Its words may also arrive in any order. For a lint name: an
   * identifier typed from memory, a handful of words with no sentence to
   * put them in order.
   * @param {string} query
   * @param {string} target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchFuzzy(query, target) {
    var needle = fold(query)
    var haystack = fold(target)
    if (needle.length === 0 || haystack.length === 0) {
      return null
    }
    // Every tier is tried, because a greedy subsequence scan does not
    // always find the best match (`ab` against `a_xab` takes `a` at 0 and
    // `b` at 4, missing the contiguous `ab` at 3) — which is why that
    // scan is the last of them rather than the only method.
    var found = matchVerbatim(needle, haystack)
    found = betterAdmitted(found, matchRespaced(needle, haystack), haystack)
    found = betterAdmitted(found, matchVariants(needle, haystack), haystack)
    found = betterAdmitted(found, matchReordered(needle, haystack), haystack)
    found = betterAdmitted(found, matchScattered(needle, haystack), haystack)
    return found && admits(found.ranges, haystack) ? found : null
  }

  /**
   * Score `query` against `target`, requiring the whole query verbatim.
   * Returns `null` when it doesn't appear. Scored by the same model as
   * `matchFuzzy`, so the two are comparable. For prose, which neither of
   * that one's freedoms suits: a paragraph a few hundred characters long
   * carries almost any scattered sequence, and almost any two words apart
   * from each other.
   * @param {string} query
   * @param {string} target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchPhrase(query, target) {
    var needle = fold(query)
    var haystack = fold(target)
    if (needle.length === 0 || haystack.length === 0) {
      return null
    }
    var found = matchVerbatim(needle, haystack)
    found = betterAdmitted(found, matchRespaced(needle, haystack), haystack)
    found = betterAdmitted(found, matchVariants(needle, haystack), haystack)
    return found && admits(found.ranges, haystack) ? found : null
  }

  /**
   * Narrow `text` to a window around its first matched range, so one long
   * paragraph can't swamp a result list. An elided end is marked with a
   * horizontal ellipsis, and the ranges come back shifted onto the window.
   * A range the window cuts through comes back cut to it, and one that
   * falls outside altogether is dropped: what the reader can see of a
   * match is marked, and a match longer than the window is still most of
   * what they are looking at.
   * @param {string} text
   * @param {number[][]} ranges
   * @param {number} limit  the longest window to keep, in characters
   * @returns {{ text: string, ranges: number[][] }}
   */
  function excerpt(text, ranges, limit) {
    if (text.length <= limit) {
      return { text: text, ranges: ranges }
    }
    var anchor = ranges.length > 0 ? ranges[0][0] : 0
    // Keep a quarter of the window ahead of the match so the reader sees
    // what it sits in, and clamp to the text's ends so a match near either
    // one still fills the whole window.
    var start = Math.max(0, Math.min(anchor - Math.floor(limit / 4), text.length - limit))
    var end = start + limit
    var slice = text.slice(start, end)
    /** @type {number[][]} */
    var shifted = []
    for (var i = 0; i < ranges.length; i++) {
      var from = Math.max(ranges[i][0], start)
      var to = Math.min(ranges[i][1], end)
      if (from >= to) {
        continue
      }
      shifted.push([from - start, to - start])
    }
    var prefix = start > 0 ? '\u2026' : ''
    if (prefix) {
      for (var j = 0; j < shifted.length; j++) {
        shifted[j][0] += prefix.length
        shifted[j][1] += prefix.length
      }
    }
    return {
      text: prefix + slice + (end < text.length ? '\u2026' : ''),
      ranges: shifted,
    }
  }

  return {
    matchFuzzy: matchFuzzy,
    matchPhrase: matchPhrase,
    excerpt: excerpt,
  }
})()
