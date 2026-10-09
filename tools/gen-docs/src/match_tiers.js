// ==========================================================================
// The tiers a query is looked for in, from the query exactly through to
// its characters scattered. Each returns one match or nothing; which tier
// wins is not decided here.
// ==========================================================================

var perfectionistMatchTiers = (function () {
  var words = perfectionistMatchText.words;
  var wordSpans = perfectionistMatchText.wordSpans;
  var stem = perfectionistMatchText.stem;
  var onlySeparators = perfectionistMatchText.onlySeparators;
  var commonPrefix = perfectionistMatchText.commonPrefix;
  var isWordStart = perfectionistMatchText.isWordStart;
  var wordEnd = perfectionistMatchText.wordEnd;
  var nextWord = perfectionistMatchText.nextWord;
  var BASE = perfectionistMatchScore.BASE;
  var RUN_BONUS = perfectionistMatchScore.RUN_BONUS;
  var opening = perfectionistMatchScore.opening;
  var blend = perfectionistMatchScore.blend;
  var betterAdmitted = perfectionistMatchAdmit.betterAdmitted;

  /**
   * Locate the whole query verbatim in the haystack. Every occurrence is
   * a candidate, and `betterAdmitted` picks between them: one worth
   * showing over one that is not, and otherwise the higher-scoring, which
   * is the occurrence opening a word wherever one does.
   * @param {string} needle    folded query
   * @param {string} haystack  folded target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchVerbatim(needle, haystack) {
    // `indexOf("")` clamps past the end rather than returning -1, so an
    // empty needle would never leave the loop.
    if (needle.length === 0) return null;
    /** @type {{ score: number, ranges: number[][] } | null} */
    var best = null;
    var at = haystack.indexOf(needle);
    while (at >= 0) {
      var raw = opening(haystack, at) + (BASE + RUN_BONUS) * (needle.length - 1);
      best = betterAdmitted(
        best,
        {
          score: blend(raw, needle.length, haystack.length),
          ranges: [[at, at + needle.length]],
        },
        haystack,
      );
      at = haystack.indexOf(needle, at + 1);
    }
    return best;
  }

  // ---- Respaced -----------------------------------------------------------

  /**
   * One haystack index per non-separator character of the query, starting
   * from `start`, or `null` when they don't all land. Skipping the
   * haystack's separators is what lets a query written without them
   * match; skipping a run of them is what lets a query written with too
   * many; and passing over the query's own costs them their score.
   * @param {string} needle    folded query
   * @param {string} haystack  folded target
   * @param {number} start
   * @returns {number[] | null}
   */
  function placeRespaced(needle, haystack, start) {
    /** @type {number[]} */
    var places = [];
    var at = start;
    for (var i = 0; i < needle.length; i++) {
      if (needle.charAt(i) === " ") continue;
      while (at < haystack.length && haystack.charAt(at) === " ") at++;
      if (haystack.charAt(at) !== needle.charAt(i)) return null;
      places.push(at);
      at++;
    }
    return places.length > 0 ? places : null;
  }

  /**
   * A position straight after the last continues a run; one the haystack's
   * separators pushed along opens a word instead, which is how a query
   * whose separators do not line up scores below one whose do. The
   * highlight is the whole span, separators included: a reader who typed a
   * phrase expects to see the phrase marked.
   * @param {number[]} places
   * @param {number} length    the query's length
   * @param {string} haystack  folded target
   * @returns {{ score: number, ranges: number[][] }}
   */
  function scorePlaces(places, length, haystack) {
    var raw = 0;
    // -2 so the first position can never read as contiguous with it.
    var previous = -2;
    for (var i = 0; i < places.length; i++) {
      if (places[i] === previous + 1) raw += BASE + RUN_BONUS;
      else raw += opening(haystack, places[i]);
      previous = places[i];
    }
    return {
      score: blend(raw, length, haystack.length),
      ranges: [[places[0], places[places.length - 1] + 1]],
    };
  }

  /**
   * Score the query against the haystack where only their separators
   * differ.
   * @param {string} needle    folded query
   * @param {string} haystack  folded target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchRespaced(needle, haystack) {
    // The query's first character that is not a separator: every
    // candidate match begins on one of its occurrences, and a query of
    // nothing but separators has none to begin on.
    var lead = 0;
    while (lead < needle.length && needle.charAt(lead) === " ") lead++;
    if (lead >= needle.length) return null;
    var head = needle.charAt(lead);
    /** @type {{ score: number, ranges: number[][] } | null} */
    var best = null;
    var at = haystack.indexOf(head);
    while (at >= 0) {
      var places = placeRespaced(needle, haystack, at);
      if (places) {
        best = betterAdmitted(best, scorePlaces(places, needle.length, haystack), haystack);
      }
      at = haystack.indexOf(head, at + 1);
    }
    return best;
  }

  // ---- Variants -----------------------------------------------------------

  /**
   * One for one, from the word starting at `start`.
   * @param {string[]} parts     the query's words
   * @param {string[]} stems     their stems, in the same order
   * @param {number} length      how many characters the query's words hold
   * @param {string} haystack    folded target
   * @param {number} start
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function alignWords(parts, stems, length, haystack, start) {
    /** @type {number[][]} */
    var ranges = [];
    var raw = 0;
    var at = start;
    for (var i = 0; i < parts.length; i++) {
      if (i > 0) {
        at = nextWord(haystack, at);
        if (at < 0) return null;
      }
      var end = wordEnd(haystack, at);
      var word = haystack.slice(at, end);
      // The target's word opens with the query's, once the ending the
      // query's would drop is allowed for — `cloned` against `cloning`,
      // and a word still being typed, three letters into `getter`.
      if (word.indexOf(stems[i]) !== 0) return null;
      var shared = commonPrefix(parts[i], word);
      raw += opening(haystack, at) + (BASE + RUN_BONUS) * (shared - 1);
      // A phrase is marked as the phrase, as the respaced tier marks it:
      // marks merge across a run of separators but not across letters, so
      // `clone` stops short of `cloning`'s `ing`. Scoring is untouched.
      var last = ranges[ranges.length - 1];
      if (last && onlySeparators(haystack, last[1], at)) last[1] = at + shared;
      else ranges.push([at, at + shared]);
      at = end;
    }
    return { score: blend(raw, length, haystack.length), ranges: ranges };
  }

  /**
   * Word for word, each haystack word opening with the query's once the
   * ending the query's would drop is allowed for. They have to align with
   * a run of *consecutive* haystack words, or this would answer any
   * paragraph carrying the same words somewhere apart from each other.
   * @param {string} needle    folded query
   * @param {string} haystack  folded target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchVariants(needle, haystack) {
    var parts = words(needle);
    if (parts.length === 0) return null;
    /** @type {string[]} */
    var stems = [];
    // The words, not the delimiters: nothing is matched against one here,
    // so charging for it would score a reader who has typed `cloned ` below
    // one who stopped at `cloned`.
    var span = 0;
    for (var i = 0; i < parts.length; i++) {
      stems.push(stem(parts[i]));
      span += parts[i].length;
    }
    /** @type {{ score: number, ranges: number[][] } | null} */
    var best = null;
    // A stem is a prefix of every word it came from, so every haystack
    // word that could align with the query's first starts with that
    // word's stem — which is one `indexOf` away.
    var at = haystack.indexOf(stems[0]);
    while (at >= 0) {
      if (isWordStart(haystack, at)) {
        best = betterAdmitted(best, alignWords(parts, stems, span, haystack, at), haystack);
      }
      at = haystack.indexOf(stems[0], at + 1);
    }
    return best;
  }

  // ---- Reordered ----------------------------------------------------------

  /**
   * @param {string} part       a word of the query
   * @param {string} haystack   folded target
   * @param {number[]} span     where the target's word begins and ends
   * @returns {{ raw: number, range: number[] }}
   */
  function wordAgainstWord(part, haystack, span) {
    var shared = commonPrefix(part, haystack.slice(span[0], span[1]));
    return {
      raw: opening(haystack, span[0]) + (BASE + RUN_BONUS) * (shared - 1),
      range: [span[0], span[0] + shared],
    };
  }

  /**
   * Give each of the query's words a different word of the target to
   * stand on, or `null` when they cannot all be housed — a query that
   * says `core` twice needs a target that does.
   *
   * The search is exhaustive, which it can afford to be: this tier runs
   * against lint names, and a lint name is a handful of words.
   * @param {string[]} parts    the query's words
   * @param {string[]} stems    their stems, in the same order
   * @param {string} haystack   folded target
   * @param {number[][]} spans  where each of the target's words sits
   * @returns {number[] | null} one span index per query word
   */
  function houseWords(parts, stems, haystack, spans) {
    /** @type {number[][]} */
    var options = [];
    for (var i = 0; i < parts.length; i++) {
      /** @type {number[]} */
      var fits = [];
      for (var j = 0; j < spans.length; j++) {
        if (haystack.slice(spans[j][0], spans[j][1]).indexOf(stems[i]) === 0) fits.push(j);
      }
      if (fits.length === 0) return null;
      options.push(fits);
    }
    /** @type {boolean[]} */
    var taken = [];
    /**
     * A way to house the query's words from `i` on, or `null` when there
     * is none. The first found is the one taken: a second housing needs
     * two of the target's words to open alike, which a lint name's words
     * rarely do, and choosing between them would move a score rather than
     * an answer.
     * @param {number} i
     * @returns {number[] | null}
     */
    function walk(i) {
      if (i === parts.length) return [];
      for (var k = 0; k < options[i].length; k++) {
        var j = options[i][k];
        if (taken[j]) continue;
        taken[j] = true;
        var rest = walk(i + 1);
        taken[j] = false;
        if (rest) return [j].concat(rest);
      }
      return null;
    }
    return walk(0);
  }

  /**
   * Score the query against the haystack with its words in any order:
   * each query word takes a different word of the target, and the target
   * may carry words the query left out, so `core_instead_of_std` answers
   * `core std instead`. A word still has to open a target word by the
   * variants tier's reading, so all this adds is the leave to arrive out
   * of order and to pass a word by. One word has no order to be out of,
   * so it starts at two.
   * @param {string} needle    folded query
   * @param {string} haystack  folded target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchReordered(needle, haystack) {
    var parts = words(needle);
    if (parts.length < 2) return null;
    var spans = wordSpans(haystack);
    if (parts.length > spans.length) return null;
    /** @type {string[]} */
    var stems = [];
    var span = 0;
    for (var i = 0; i < parts.length; i++) {
      stems.push(stem(parts[i]));
      span += parts[i].length;
    }
    var housed = houseWords(parts, stems, haystack, spans);
    if (!housed) return null;
    // Marked where the target reads them, not where the query typed them.
    var chosen = housed.slice().sort(function (left, right) {
      return left - right;
    });
    var raw = 0;
    /** @type {number[][]} */
    var ranges = [];
    for (var k = 0; k < chosen.length; k++) {
      var at = spans[chosen[k]][0];
      var found = wordAgainstWord(parts[housed.indexOf(chosen[k])], haystack, spans[chosen[k]]);
      raw += found.raw;
      var last = ranges[ranges.length - 1];
      if (last && onlySeparators(haystack, last[1], at)) last[1] = found.range[1];
      else ranges.push(found.range);
    }
    return { score: blend(raw, span, haystack.length), ranges: ranges };
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

  return {
    matchVerbatim: matchVerbatim,
    placeRespaced: placeRespaced,
    scorePlaces: scorePlaces,
    matchRespaced: matchRespaced,
    alignWords: alignWords,
    matchVariants: matchVariants,
    wordAgainstWord: wordAgainstWord,
    houseWords: houseWords,
    matchReordered: matchReordered,
    matchScattered: matchScattered,
  };
})();
