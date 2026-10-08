// ==========================================================================
// What a match earns. Every number the scoring uses lives here, and they
// order results against each other and nothing else, so they can be
// re-tuned without changing which targets a query reaches.
// ==========================================================================

var perfectionistMatchScore = (function () {
  var isWordStart = perfectionistMatchText.isWordStart

  // BASE is the smallest on purpose: a match owes its score to landing
  // contiguously or on a word boundary, not to occurring at all.
  var BASE = 0.4

  var RUN_BONUS = 1

  var WORD_BONUS = 0.6

  var START_BONUS = 0.3

  // How the two score components are blended. `quality` carries the match,
  // `coverage` only breaks its ties, so the weights are lopsided; they sum
  // to 1 so the result stays in 0..1.
  var QUALITY_WEIGHT = 0.95

  var COVERAGE_WEIGHT = 0.05

  /**
   * What a character earns for where it sits, when it opens a run rather
   * than continuing one: most at the target's first character, less at a
   * word's, and only BASE anywhere else. Opening the name is not the same
   * as opening a word inside it — without the difference, `n` would score
   * `excessive_nesting` and `named_prelude_imports` alike.
   * @param {string} haystack  folded target
   * @param {number} index
   * @returns {number}
   */
  function opening(haystack, index) {
    if (index === 0) return BASE + WORD_BONUS + START_BONUS
    if (isWordStart(haystack, index)) return BASE + WORD_BONUS
    return BASE
  }

  /**
   * What a query of `length` characters earns at best: every one of them
   * contiguous from the target's first. Dividing by it is what makes two
   * queries' scores comparable.
   * @param {number} length
   * @returns {number}
   */
  function idealScore(length) {
    return BASE + WORD_BONUS + START_BONUS + (BASE + RUN_BONUS) * (length - 1)
  }

  /**
   * @param {number} raw      score earned by the match
   * @param {number} length   the query's length
   * @param {number} extent   the target's length
   * @returns {number}
   */
  function blend(raw, length, extent) {
    var quality = raw / idealScore(length)
    // `coverage` is the fraction of the target the query accounts for, and
    // a query can now be longer than what it matched — a separator the
    // target spells differently is a character the query carries and the
    // target does not — so the ratio is held to a fraction.
    var coverage = Math.min(1, length / Math.max(extent, 1))
    return QUALITY_WEIGHT * quality + COVERAGE_WEIGHT * coverage
  }

  // Two tiers placing the same characters in the same places earn the same
  // total by different arithmetic — one character at a time against one
  // multiplication per word — which doubles do not always agree on to the
  // last bit. Far below the smallest difference the scoring can mean, so
  // it absorbs that noise and nothing else.
  var SCORE_EPSILON = 1e-9

  /**
   * Whichever of two matches scores higher, where either may be absent. A
   * tie keeps the first, which is the stronger tier, so which tier
   * answers a tie is a rule and not a rounding. No query now tells the
   * two apart — tiers that tie place the same characters, and the
   * ranges they hand back agree — so this settles the order rather than
   * any answer.
   * @param {{ score: number, ranges: number[][] } | null} left
   * @param {{ score: number, ranges: number[][] } | null} right
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function better(left, right) {
    if (!left) return right
    if (!right) return left
    return right.score > left.score + SCORE_EPSILON ? right : left
  }

  return {
    BASE: BASE,
    RUN_BONUS: RUN_BONUS,
    WORD_BONUS: WORD_BONUS,
    START_BONUS: START_BONUS,
    QUALITY_WEIGHT: QUALITY_WEIGHT,
    COVERAGE_WEIGHT: COVERAGE_WEIGHT,
    opening: opening,
    idealScore: idealScore,
    blend: blend,
    SCORE_EPSILON: SCORE_EPSILON,
    better: better,
  }
})()
