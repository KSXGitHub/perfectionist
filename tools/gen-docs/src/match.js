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
// as long as they occur in order. `matchPhrase` does not: it keeps the
// query's words whole. Which one a caller wants follows from what it is
// matching against:
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
// Everything short of scattering is shared, so a different spelling of a
// separator and a different ending of a word are met by both. The two also
// share one scoring model, so their scores stay comparable and a caller
// matching both can rank the results against each other.
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
// ---- When the query is not there whole ------------------------------------
//
// A reader's query is as often a near miss as a substring of what they are
// after, so a match is looked for in tiers. A verbatim one ends the
// search; below it every remaining tier is tried and the best-scoring one
// wins:
//
//   1. Verbatim — the query occurs in the target exactly. Nothing beats
//      this.
//   2. Respaced — the query occurs except that its separators do not line
//      up: one the query wrote is missing from the target, one the target
//      carries is absent from the query, or a run of either stands where a
//      single one does. This is what finds `thiserror_usage` for
//      `this error`, and a statement reading "Flags closure parameters"
//      for `flagsclosureparameters` or for `Flags  closure   parameters`.
//   3. Variants — the query's words align one for one with a run of the
//      target's consecutive words, each matching by its stem where it does
//      not match in full, so `clone_getter` and `cloned_getter` both reach
//      `cloning_getter`. Only the letters the two words share are scored
//      and marked.
//   4. Scattered — the query's characters occur in order with anything at
//      all between them. Names only, per the section above.
//
// Every tier below the first is weaker than it by construction rather than
// by a penalty: each scores only the characters it actually placed, while
// the divisor it is scored against counts the whole query. A separator the
// target does not spell the same way, and the tail of a word the target
// ends differently, are characters the reader typed that earn nothing. So
// a near miss cannot reach what an exact match earns, and the thresholds
// at the bottom of this file apply to all four tiers unchanged.
//
// Greedy left-to-right subsequence scanning does not always find the
// best-scoring match (`ab` against `a_xab` takes `a` at 0 and `b` at 4,
// missing the contiguous `ab` at 3). That is why the verbatim tier is
// tried first and exactly — scanning the occurrences of the whole query
// and keeping the one that opens a word, else the first — and why the
// subsequence scan is the last resort rather than the only method.
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
    // `coverage` is the fraction of the target the query accounts for, and
    // a query can now be longer than what it matched — a separator the
    // target spells differently is a character the query carries and the
    // target does not — so the ratio is held to a fraction.
    var coverage = Math.min(1, length / Math.max(extent, 1));
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

  // ---- Words ------------------------------------------------------------
  //
  // A word, to the variants tier, is a run of letters and digits. Both
  // strings are folded by the time it runs, so every separator reads as
  // the space it became, and the punctuation a paragraph carries — a
  // comma, a full stop, a bracket — bounds a word without belonging to
  // one.

  /**
   * @param {string} ch
   * @returns {boolean}
   */
  function isAlnum(ch) {
    return (ch >= "a" && ch <= "z") || (ch >= "0" && ch <= "9");
  }

  /**
   * Where the word starting at `from` ends.
   * @param {string} text
   * @param {number} from
   * @returns {number}
   */
  function wordEnd(text, from) {
    var at = from;
    while (at < text.length && isAlnum(text.charAt(at))) at++;
    return at;
  }

  /**
   * Where the first word at or after `from` starts, or -1 when none does.
   * @param {string} text
   * @param {number} from
   * @returns {number}
   */
  function nextWord(text, from) {
    var at = from;
    while (at < text.length && !isAlnum(text.charAt(at))) at++;
    return at < text.length ? at : -1;
  }

  /**
   * `text`'s words, in order.
   * @param {string} text
   * @returns {string[]}
   */
  function words(text) {
    /** @type {string[]} */
    var out = [];
    var at = nextWord(text, 0);
    while (at >= 0) {
      var end = wordEnd(text, at);
      out.push(text.slice(at, end));
      at = nextWord(text, end);
    }
    return out;
  }

  /**
   * How many characters two strings share from the front.
   * @param {string} left
   * @param {string} right
   * @returns {number}
   */
  function commonPrefix(left, right) {
    var limit = Math.min(left.length, right.length);
    var at = 0;
    while (at < limit && left.charAt(at) === right.charAt(at)) at++;
    return at;
  }

  // ---- Stems ------------------------------------------------------------

  // The shortest word a suffix comes off, and the shortest stem left
  // behind. `bed` is too short to take an ending off at all, and `doing`
  // would leave too little of itself to tell from another word.
  var MIN_STEM_WORD = 4;
  var MIN_STEM = 3;

  /**
   * Does `text` end with `suffix`? Spelled out rather than
   * `String.prototype.endsWith`, which the page's oldest engines predate.
   * @param {string} text
   * @param {string} suffix
   * @returns {boolean}
   */
  function endsWith(text, suffix) {
    var at = text.length - suffix.length;
    return at >= 0 && text.indexOf(suffix, at) === at;
  }

  /**
   * Drop one of a doubled final consonant, which is what English put
   * there when the ending went on: `getting` leaves `gett`, and the word
   * behind it is `get`. An `ll`, `ss` or `zz` is the word's own doubling
   * — `fall`, `pass` — and stays.
   * @param {string} word
   * @returns {string}
   */
  function undouble(word) {
    if (word.length - 1 < MIN_STEM) return word;
    var last = word.charAt(word.length - 1);
    if (last !== word.charAt(word.length - 2)) return word;
    if (last === "l" || last === "s" || last === "z") return word;
    return word.slice(0, word.length - 1);
  }

  /**
   * The stem a word shares with its variants: `cloning`, `cloned`,
   * `clones` and `clone` all come back `clon`, which is what lets a
   * reader who types one of them find a lint named with another.
   *
   * A stripper over the regular English endings and nothing more — no
   * dictionary, no irregular forms, and no ending that rewrites the word
   * rather than extending it, so `getter` and `getting` stay two words.
   * Every rule takes characters off the end only, which leaves a stem
   * that is always a prefix of the word it came from; `matchVariants`
   * is built on that.
   * @param {string} word
   * @returns {string}
   */
  function stem(word) {
    if (word.length < MIN_STEM_WORD) return word;
    var out = word;
    // A plural or a third person. An `ss` or a `us` is neither: `pass`
    // and `status` end that way on their own account.
    if (
      endsWith(out, "s") &&
      !endsWith(out, "ss") &&
      !endsWith(out, "us") &&
      out.length - 1 >= MIN_STEM
    ) {
      out = out.slice(0, out.length - 1);
    }
    if (endsWith(out, "ing") && out.length - 3 >= MIN_STEM) {
      out = undouble(out.slice(0, out.length - 3));
    } else if (endsWith(out, "ed") && out.length - 2 >= MIN_STEM) {
      out = undouble(out.slice(0, out.length - 2));
    }
    // The `e` an `-ing` or an `-ed` form drops anyway, so `clone` is met
    // where `cloning` and `cloned` already are.
    if (endsWith(out, "e") && out.length - 1 >= MIN_STEM) {
      out = out.slice(0, out.length - 1);
    }
    return out;
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
   * The score and the highlight for a run of matched haystack positions.
   * A position directly after the last one continues a run; one the
   * haystack's separators pushed along opens a word instead, which is how
   * a query whose separators don't line up scores below one whose do.
   *
   * The highlight is the whole span, the separators it stepped over
   * included: a reader who typed a phrase expects to see the phrase
   * marked, not its words marked one at a time.
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
      if (places) best = better(best, scorePlaces(places, needle.length, haystack));
      at = haystack.indexOf(head, at + 1);
    }
    return best;
  }

  // ---- Variants -----------------------------------------------------------

  /**
   * Align the query's words to the haystack's, one for one, from the word
   * starting at `start`.
   * @param {string[]} parts     the query's words
   * @param {string[]} stems     their stems, in the same order
   * @param {number} length      the query's length
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
      if (stem(word) !== stems[i]) return null;
      var shared = commonPrefix(parts[i], word);
      raw += opening(haystack, at) + (BASE + RUN_BONUS) * (shared - 1);
      ranges.push([at, at + shared]);
      at = end;
    }
    return { score: blend(raw, length, haystack.length), ranges: ranges };
  }

  /**
   * Score the query against the haystack word for word, taking a word
   * whose stem matches where the word itself does not. The query's words
   * have to align with a run of consecutive haystack words, which is what
   * keeps this from answering a paragraph that merely carries the same
   * words somewhere apart from each other.
   * @param {string} needle    folded query
   * @param {string} haystack  folded target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchVariants(needle, haystack) {
    var parts = words(needle);
    if (parts.length === 0) return null;
    /** @type {string[]} */
    var stems = [];
    for (var i = 0; i < parts.length; i++) stems.push(stem(parts[i]));
    /** @type {{ score: number, ranges: number[][] } | null} */
    var best = null;
    // A stem is a prefix of every word it came from, so every haystack
    // word that could align with the query's first starts with that
    // word's stem — which is one `indexOf` away.
    var at = haystack.indexOf(stems[0]);
    while (at >= 0) {
      if (isWordStart(haystack, at)) {
        best = better(best, alignWords(parts, stems, needle.length, haystack, at));
      }
      at = haystack.indexOf(stems[0], at + 1);
    }
    return best;
  }

  /**
   * Whichever of two matches scores higher, where either may be absent.
   * @param {{ score: number, ranges: number[][] } | null} left
   * @param {{ score: number, ranges: number[][] } | null} right
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function better(left, right) {
    if (!left) return right;
    if (!right) return left;
    return right.score > left.score ? right : left;
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
    return better(
      better(matchRespaced(needle, haystack), matchVariants(needle, haystack)),
      matchScattered(needle, haystack)
    );
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
    var verbatim = matchVerbatim(needle, haystack);
    if (verbatim) return verbatim;
    return better(matchRespaced(needle, haystack), matchVariants(needle, haystack));
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
