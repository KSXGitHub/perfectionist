// ============================================================================
// Query matching: scoring a query against a string, and narrowing a long
// string to the part that matched.
//
// Nothing here touches the DOM, and nothing here knows what a lint is. It
// is the arithmetic the catalogue's search and its filter boxes are both
// built on, and it is the one file of the page's JavaScript that can be
// loaded and exercised outside a browser — see tools/gen-docs/tests/.
// Painting a match onto the page is highlight.js; ranking rules by their
// matches is rank.js.
//
// `perfectionistMatch` is a global because the page loads classic scripts,
// not modules — the rest of the catalogue's JS targets engines that predate
// `import`, and a `<script type="module">` would also be skipped entirely
// by those engines rather than degrading. The object is built inside an
// IIFE so the helpers behind it stay private, and it is the one name this
// file adds to the global scope.
//
// ---- Two kinds of match ---------------------------------------------------
//
// `matchFuzzy` lets the query's characters be scattered through the target,
// as long as they occur in order. `matchPhrase` requires the whole query
// verbatim. Which one a caller wants follows from what it is matching
// against:
//
//   * A lint name is an identifier, and a reader types it from memory —
//     skipping the underscores (`bareurl`), or typing a word's initials
//     (`bur`). Scattering is the point, so names get `matchFuzzy`.
//   * Prose is words. A reader searching it types some of them, never
//     their initials — and a paragraph a few hundred characters long
//     contains almost any short sequence of letters in order, so letting
//     the characters scatter there finds a high-scoring match in nearly
//     every rule on the page. Prose gets `matchPhrase`.
//
// The two share one scoring model, so their scores stay comparable and a
// caller matching both can rank the results against each other.
//
// Both compare case-insensitively, and both treat `_`, `-` and a space as
// the same character, because one thing goes by all three spellings here:
// the lint `bare_url` is `bare-url` in its own page fragment and "bare
// URL" in its statement. A reader who types any of them means the lint.
// Folding maps each such character to one space rather than collapsing
// runs of them, so the folded string indexes exactly like the original and
// the ranges a match reports can be used to highlight the text the caller
// passed in.
//
// ---- The scoring model ----------------------------------------------------
//
// Each matched character earns:
//
//   * BASE, always;
//   * RUN_BONUS when it directly follows the previous matched character, so
//     a contiguous run outranks the same characters scattered about;
//   * WORD_BONUS, instead of RUN_BONUS, when it opens a word — index 0, or
//     anything after a separator. `_` is a separator, which is what makes
//     `bur` score well against `bare_url` and lets a reader type the
//     initials of a snake_case name.
//   * START_BONUS on top of that when it is the target's own first
//     character, because opening the name is not the same as opening a
//     word inside it. Without it, the query `n` scores `excessive_nesting`
//     and `named_prelude_imports` identically on quality — both open a
//     word — and the only thing left to separate them is the coverage term
//     below, which prefers the shorter name. The rule that merely contains
//     an `n` then outranks the one that starts with it. The bonus is a
//     fixed amount spread over the whole query, so it decides a
//     one-character query and fades as the query grows specific enough to
//     decide itself.
//
// The total is divided by the best score the query could possibly earn
// (every character contiguous, from the target's first), so the resulting
// `quality` lands in 0..1 whatever the query's length — one threshold then
// works for every query. A small `coverage` term (the
// fraction of the target the query accounts for) breaks ties towards the
// shorter target, so `bare_url` outranks `bare_identifier_reference` for
// the query `bare`. Its weight is kept low on purpose: it is there to
// order two matches of equal quality, and a target's length must not be
// able to carry a weak match past the threshold or hold a strong one
// back.
//
// Greedy left-to-right subsequence scanning does not always find the
// best-scoring match (`ab` against `a_xab` takes `a` at 0 and `b` at 4,
// missing the contiguous `ab` at 3). Rather than search exhaustively,
// `matchFuzzy` tries the verbatim match first and exactly — scanning the
// occurrences of the whole query and keeping the one that opens a word,
// else the first — and falls back to the subsequence scan only for a query
// that appears nowhere whole. `matchPhrase` is that first half alone.
// ============================================================================

var perfectionistMatch = (function () {
  // Per-character score components. BASE is deliberately the smallest of
  // them: a match owes its score to landing contiguously or on a word
  // boundary, not to the bare fact that the character occurs somewhere.
  var BASE = 0.4;
  var RUN_BONUS = 1;
  var WORD_BONUS = 0.6;
  var START_BONUS = 0.3;

  // How the two score components are blended. `quality` carries the match,
  // `coverage` only breaks its ties, so the weights are lopsided; they sum
  // to 1 so the result stays in 0..1.
  var QUALITY_WEIGHT = 0.95;
  var COVERAGE_WEIGHT = 0.05;

  /**
   * Case-fold a string and flatten its separators, as the file header
   * describes. One character in, one character out, so the result indexes
   * exactly like the input.
   * @param {string} text
   * @returns {string}
   */
  function fold(text) {
    return text.toLowerCase().replace(/[-_\s]/g, " ");
  }

  /**
   * Is the character at `index` the start of a word? Index 0 is, and so is
   * anything whose predecessor is not alphanumeric — a space, a `:`, a
   * bracket. The haystack is already folded by the time this runs, so the
   * test need not consider upper case, and the separators the fold
   * flattened all read as the space they became.
   * @param {string} haystack
   * @param {number} index
   * @returns {boolean}
   */
  function isWordStart(haystack, index) {
    if (index <= 0) return true;
    var previous = haystack.charAt(index - 1);
    return !(previous >= "a" && previous <= "z") && !(previous >= "0" && previous <= "9");
  }

  /**
   * What a character earns for where it sits, when it opens a run rather
   * than continuing one: most at the target's first character, less at a
   * word's, and only BASE anywhere else. See the file header for why the
   * two kinds of opening are not worth the same.
   * @param {string} haystack  folded target
   * @param {number} index
   * @returns {number}
   */
  function opening(haystack, index) {
    if (index === 0) return BASE + WORD_BONUS + START_BONUS;
    if (isWordStart(haystack, index)) return BASE + WORD_BONUS;
    return BASE;
  }

  /**
   * The score a query of `length` characters earns when every one of them
   * matches contiguously from the target's first character. Dividing by
   * this is what makes scores comparable across queries of different
   * lengths.
   * @param {number} length
   * @returns {number}
   */
  function idealScore(length) {
    return BASE + WORD_BONUS + START_BONUS + (BASE + RUN_BONUS) * (length - 1);
  }

  /**
   * Blend the two score components into the 0..1 value callers compare
   * against a threshold.
   * @param {number} raw      score earned by the match
   * @param {number} length   the query's length
   * @param {number} extent   the target's length
   * @returns {number}
   */
  function blend(raw, length, extent) {
    var quality = raw / idealScore(length);
    var coverage = length / Math.max(extent, 1);
    return QUALITY_WEIGHT * quality + COVERAGE_WEIGHT * coverage;
  }

  /**
   * Locate the whole query verbatim in the haystack, preferring an
   * occurrence that opens a word over an earlier one that doesn't.
   * @param {string} needle    folded query
   * @param {string} haystack  folded target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchVerbatim(needle, haystack) {
    var best = -1;
    var at = haystack.indexOf(needle);
    while (at >= 0) {
      if (best < 0) best = at;
      if (isWordStart(haystack, at)) {
        best = at;
        break;
      }
      at = haystack.indexOf(needle, at + 1);
    }
    if (best < 0) return null;
    var raw = opening(haystack, best) + (BASE + RUN_BONUS) * (needle.length - 1);
    return {
      score: blend(raw, needle.length, haystack.length),
      ranges: [[best, best + needle.length]],
    };
  }

  /**
   * Walk the haystack once, taking the first remaining occurrence of each
   * query character in turn.
   * @param {string} needle    folded query
   * @param {string} haystack  folded target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchScattered(needle, haystack) {
    /** @type {number[][]} */
    var ranges = [];
    var raw = 0;
    var cursor = 0;
    // -2 so the first character can never read as contiguous with it.
    var previous = -2;
    for (var i = 0; i < needle.length; i++) {
      var found = haystack.indexOf(needle.charAt(i), cursor);
      if (found < 0) return null;
      if (found === previous + 1) {
        raw += BASE + RUN_BONUS;
        // Extend the run in place rather than opening a second range, so
        // the highlight renders one <mark> per contiguous stretch.
        ranges[ranges.length - 1][1] = found + 1;
      } else {
        raw += opening(haystack, found);
        ranges.push([found, found + 1]);
      }
      previous = found;
      cursor = found + 1;
    }
    return {
      score: blend(raw, needle.length, haystack.length),
      ranges: ranges,
    };
  }

  /**
   * Score `query` against `target`, allowing the query's characters to be
   * scattered through it as long as they occur in order. Returns the score
   * together with the half-open `[start, end)` character ranges of
   * `target` that matched, or `null` when they don't all occur in order.
   * For a lint name; see the file header for why prose wants the other
   * one.
   * @param {string} query
   * @param {string} target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchFuzzy(query, target) {
    var needle = fold(query);
    var haystack = fold(target);
    if (needle.length === 0 || haystack.length === 0) return null;
    var verbatim = matchVerbatim(needle, haystack);
    if (verbatim) return verbatim;
    return matchScattered(needle, haystack);
  }

  /**
   * Score `query` against `target`, requiring the whole query verbatim.
   * Returns `null` when it doesn't appear. Scored by the same model as
   * `matchFuzzy`, so the two are comparable. For prose; see the file
   * header.
   * @param {string} query
   * @param {string} target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchPhrase(query, target) {
    var needle = fold(query);
    var haystack = fold(target);
    if (needle.length === 0 || haystack.length === 0) return null;
    return matchVerbatim(needle, haystack);
  }

  /**
   * Narrow `text` to a window around its first matched range, so one long
   * paragraph can't swamp a result list. An elided end is marked with a
   * horizontal ellipsis, and the ranges come back shifted onto the window.
   * Ranges that fall outside it are dropped.
   * @param {string} text
   * @param {number[][]} ranges
   * @param {number} limit  the longest window to keep, in characters
   * @returns {{ text: string, ranges: number[][] }}
   */
  function excerpt(text, ranges, limit) {
    if (text.length <= limit) return { text: text, ranges: ranges };
    var anchor = ranges.length > 0 ? ranges[0][0] : 0;
    // Keep a quarter of the window ahead of the match so the reader sees
    // what it sits in, and clamp to the text's ends so a match near either
    // one still fills the whole window.
    var start = Math.max(0, Math.min(anchor - Math.floor(limit / 4), text.length - limit));
    var end = start + limit;
    var slice = text.slice(start, end);
    /** @type {number[][]} */
    var shifted = [];
    for (var i = 0; i < ranges.length; i++) {
      if (ranges[i][0] < start || ranges[i][1] > end) continue;
      shifted.push([ranges[i][0] - start, ranges[i][1] - start]);
    }
    var prefix = start > 0 ? "\u2026" : "";
    if (prefix) {
      for (var j = 0; j < shifted.length; j++) {
        shifted[j][0] += prefix.length;
        shifted[j][1] += prefix.length;
      }
    }
    return {
      text: prefix + slice + (end < text.length ? "\u2026" : ""),
      ranges: shifted,
    };
  }

  return {
    matchFuzzy: matchFuzzy,
    matchPhrase: matchPhrase,
    excerpt: excerpt,
    // Below this score a match is noise rather than a result. Both bounds
    // sit above what a scattered match earns when the query's characters
    // merely happen to occur in order, and below what a verbatim run earns
    // even mid-word — the gap the scoring model exists to open. The filter
    // boxes hold the tighter of the two, as one constant, because they
    // must agree with each other. The search's is a little looser: it
    // offers a ranked ten rather than a filtered list, so a near miss
    // there costs the reader a glance rather than a wrong answer.
    FILTER_MIN_SCORE: 0.65,
    SEARCH_MIN_SCORE: 0.6,
  };
})();
