// ============================================================================
// Query matching: scoring a query against a string, and narrowing a long
// string to the part that matched. Nothing here touches the DOM and
// nothing here knows what a lint is, which is what lets it run with no
// browser around it; painting a match onto the page is highlight.js.
//
// `perfectionistMatch` is a global because the page loads classic scripts
// rather than modules: its engines predate `import`, and a
// `<script type="module">` would be skipped outright rather than degrade.
//
// ---- Two kinds of match ---------------------------------------------------
//
// `matchFuzzy` lets the query come apart — its characters scattered
// through the target in order, its words in any order at all — where
// `matchPhrase` lets neither. A lint name gets the first: it is an
// identifier typed from memory, a handful of words with no sentence to put
// them in order. Prose gets the second, because a paragraph a few hundred
// characters long carries almost any scattered sequence and almost any two
// words apart from each other, so either freedom would answer nearly every
// rule on the page. Everything short of those two is shared, and so is the
// scoring model, so a caller matching both can rank the results together.
//
// ---- How well it matched --------------------------------------------------
//
// A character earns more for continuing a run than for standing alone,
// more for opening a word than for landing inside one, and most for
// opening the target itself; the total is divided by what the query could
// have earned at best, so the score is a fraction whatever its length. The
// constants below carry the detail.
//
// Every one of those numbers orders matches against each other and does
// nothing else — see the next section — so they can be re-tuned freely.
//
// ---- When the query is not there whole ------------------------------------
//
// A query is as often a near miss as a substring, so a match is looked for
// in tiers: verbatim, respaced, variants, reordered, scattered, each
// documented at its own function. Every tier is tried and the best-scoring
// one worth showing wins, between tiers and between the placements of one
// tier alike, so a coincidence that scores higher cannot hide the real
// match behind it.
//
// A tier is weaker than the one above by construction rather than by a
// penalty: each scores only the characters it placed. What it counts as
// the query differs with what it can place — the first two place every
// character the query holds, separators and all, while the word-wise tiers
// never match a target character against a separator and are charged for
// the query's words alone. Charging them for the delimiters too would mean
// a reader who has typed `cloned ` and is about to type `getter` scoring
// below one who stopped at `cloned`: a result vanishing halfway through
// being typed.
//
// Greedy left-to-right subsequence scanning does not always find the
// best-scoring match (`ab` against `a_xab` takes `a` at 0 and `b` at 4,
// missing the contiguous `ab` at 3), which is why the tiers exist at all
// and why that scan is the last of them rather than the only method.
//
// ---- Worth showing --------------------------------------------------------
//
// A different question from how well, and this file keeps the two apart.
// Answering it with a bar on the score would tie what a reader can find to
// how the weights happen to be tuned, and would let one more character
// push a result back over a bar it had fallen under, so it leaves the list
// and returns. `admits` therefore reads no score, only where the match
// landed, and the rules it applies are documented there.
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
   * exactly like the input — which every range handed back depends on,
   * since they index the folded string and are read against the original.
   *
   * Lower-casing is not length-preserving for every character: `İ` comes
   * back as two code units, and one of those ahead of a match would shift
   * every range after it. So where it grows the string, the case is left
   * as it stands and only the separators are flattened. A query typed in
   * another case then misses such a target, which is the smaller of the
   * two wrongs — the other is a highlight on text the reader never typed.
   * @param {string} text
   * @returns {string}
   */
  function fold(text) {
    var lowered = text.toLowerCase();
    return (lowered.length === text.length ? lowered : text).replace(/[-_\s]/g, " ");
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
   * Blend the two score components into the 0..1 value results are
   * ordered by.
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
    // empty needle would never leave the loop below.
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
   * Where each of `text`'s words begins and ends, in order.
   * @param {string} text
   * @returns {number[][]}
   */
  function wordSpans(text) {
    /** @type {number[][]} */
    var out = [];
    var at = nextWord(text, 0);
    while (at >= 0) {
      var end = wordEnd(text, at);
      out.push([at, end]);
      at = nextWord(text, end);
    }
    return out;
  }

  /**
   * `text`'s words, in order.
   * @param {string} text
   * @returns {string[]}
   */
  function words(text) {
    var spans = wordSpans(text);
    /** @type {string[]} */
    var out = [];
    for (var i = 0; i < spans.length; i++) out.push(text.slice(spans[i][0], spans[i][1]));
    return out;
  }

  /**
   * Does nothing but separators stand between `from` and `to`?
   * @param {string} text
   * @param {number} from
   * @param {number} to
   * @returns {boolean}
   */
  function onlySeparators(text, from, to) {
    for (var at = from; at < to; at++) {
      if (text.charAt(at) !== " ") return false;
    }
    return true;
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
   * The regular English endings and nothing more — no dictionary, no
   * irregular forms, and no ending that rewrote the word rather than
   * extending it, so `getter` stems to itself and never reaches
   * `getting`. Every rule takes characters off the end only, so a stem is
   * always a prefix of the word it came from, which `matchVariants` is
   * built on.
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
      if (places) {
        best = betterAdmitted(best, scorePlaces(places, needle.length, haystack), haystack);
      }
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
      // query's would drop is allowed for. Equal stems are the case this
      // started from (`cloned` against `cloning`); the looser test also
      // takes a word still being typed, where the reader is three letters
      // into `getter` and has the stem `get`.
      if (word.indexOf(stems[i]) !== 0) return null;
      var shared = commonPrefix(parts[i], word);
      raw += opening(haystack, at) + (BASE + RUN_BONUS) * (shared - 1);
      // A phrase is marked as the phrase, the same as the respaced tier
      // marks it: where only separators stand between this word and the
      // last, the two marks are one. A gap holding letters — the `ing` of
      // `cloning` that `clone` stopped short of — keeps them apart, so
      // what is marked stays what was matched. Scoring is untouched: the
      // word still opens a word rather than continuing a run.
      var last = ranges[ranges.length - 1];
      if (last && onlySeparators(haystack, last[1], at)) last[1] = at + shared;
      else ranges.push([at, at + shared]);
      at = end;
    }
    return { score: blend(raw, length, haystack.length), ranges: ranges };
  }

  /**
   * Score the query against the haystack word for word, each haystack
   * word opening with the query's once the ending the query's word would
   * drop is allowed for. The query's words have to align with a run of
   * consecutive haystack words, which is what keeps this from answering a
   * paragraph that merely carries the same words somewhere apart from
   * each other.
   * @param {string} needle    folded query
   * @param {string} haystack  folded target
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function matchVariants(needle, haystack) {
    var parts = words(needle);
    if (parts.length === 0) return null;
    /** @type {string[]} */
    var stems = [];
    // The query's words and not the delimiters between them, per the
    // file header: no target character is ever matched against one here.
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
   * What one of the query's words earns against one of the target's, and
   * how much of the target's it marks.
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
     * an answer. It hands its answer back rather than keeping it in the
     * enclosing scope, which TypeScript cannot follow across a call — the
     * same reason the page's other scripts bind their guarded elements to
     * locals.
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

  // ---- Admission ----------------------------------------------------------

  // How many of a match's runs may begin inside a word. A reader typing a
  // name from memory opens words wherever they like — that is what finds
  // `bare_url` for `bur` — but a run beginning inside a word is a letter
  // that landed where it happened to occur rather than where the reader
  // aimed it. One of those is a slip of the fingers, and keeping it is
  // what still finds `excessive_nesting` for `excessive_nestng`. Two is
  // the characters falling where they may, which is how `bare` would
  // otherwise answer `needless_borrowed_parameters`.
  var SLIPS_ALLOWED = 1;

  /**
   * Where the word holding `index` begins.
   * @param {string} haystack
   * @param {number} index
   * @returns {number}
   */
  function wordStartBefore(haystack, index) {
    var at = index;
    while (at > 0 && isAlnum(haystack.charAt(at - 1))) at--;
    return at;
  }

  /**
   * Did the reader aim this run, or did it land where it happened to? It
   * is aimed when it opens a word, and also when it finishes one and
   * holds more of that word than it skipped: `error` is the back five of
   * `thiserror`'s nine, so a reader who types it has typed one of the
   * words that identifier ran together — where one who types the `e` that
   * `single` ends in has typed a letter that happens to fall last.
   * @param {string} haystack
   * @param {number[]} range
   * @returns {boolean}
   */
  function aimed(haystack, range) {
    var start = range[0];
    if (isWordStart(haystack, start)) return true;
    if (isAlnum(haystack.charAt(range[1]))) return false;
    return range[1] - start >= start - wordStartBefore(haystack, start);
  }

  /**
   * Is this match worth showing at all?
   *
   * Nothing here reads the score, and that is the point: a score orders
   * matches against each other, where only the differences matter and the
   * numbers are free to be re-tuned, while this decides whether a reader
   * sees the target at all, where an answer that moves under re-tuning is
   * an answer nobody can rely on. See the file header.
   * @param {number[][]} ranges  the match's runs, in order, at least one
   * @param {string} haystack    folded target
   * @returns {boolean}
   */
  function admits(ranges, haystack) {
    if (!aimed(haystack, ranges[0])) return false;
    var slips = 0;
    for (var i = 1; i < ranges.length; i++) {
      if (isWordStart(haystack, ranges[i][0])) continue;
      slips++;
      if (slips > SLIPS_ALLOWED) return false;
    }
    return true;
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
   * @param {{ score: number, ranges: number[][] } | null} left
   * @param {{ score: number, ranges: number[][] } | null} right
   * @param {string} haystack
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function betterAdmitted(left, right, haystack) {
    if (!left) return right;
    if (!right) return left;
    var leftShown = admits(left.ranges, haystack);
    if (leftShown !== admits(right.ranges, haystack)) return leftShown ? left : right;
    return better(left, right);
  }

  // Two tiers placing the same characters in the same places earn the same
  // total by different arithmetic — one character at a time against one
  // multiplication per word — which doubles do not always agree on to the
  // last bit. Far below the smallest difference the scoring can mean, so
  // it absorbs that noise and nothing else.
  var SCORE_EPSILON = 1e-9;

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
    if (!left) return right;
    if (!right) return left;
    return right.score > left.score + SCORE_EPSILON ? right : left;
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
    var found = matchVerbatim(needle, haystack);
    found = betterAdmitted(found, matchRespaced(needle, haystack), haystack);
    found = betterAdmitted(found, matchVariants(needle, haystack), haystack);
    found = betterAdmitted(found, matchReordered(needle, haystack), haystack);
    found = betterAdmitted(found, matchScattered(needle, haystack), haystack);
    return found && admits(found.ranges, haystack) ? found : null;
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
    var found = matchVerbatim(needle, haystack);
    found = betterAdmitted(found, matchRespaced(needle, haystack), haystack);
    found = betterAdmitted(found, matchVariants(needle, haystack), haystack);
    return found && admits(found.ranges, haystack) ? found : null;
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
      var from = Math.max(ranges[i][0], start);
      var to = Math.min(ranges[i][1], end);
      if (from >= to) continue;
      shifted.push([from - start, to - start]);
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
  };
})();
