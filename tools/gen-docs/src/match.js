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
// `quality` lands in 0..1 whatever the query's length, and two matches of
// different queries stay comparable. A small `coverage` term (the
// fraction of the target the query accounts for) breaks ties towards the
// shorter target, so `bare_url` outranks `bare_identifier_reference` for
// the query `bare`. Its weight is kept low on purpose: it is there to
// order two matches of equal quality, not to decide either on its own.
//
// Every number here orders matches against each other and nothing else.
// What is worth showing at all is decided without reading any of them —
// see the next section but one — so these can be re-tuned freely, which
// is the point of keeping the two apart.
//
// ---- When the query is not there whole ------------------------------------
//
// A reader's query is as often a near miss as a substring of what they are
// after, so a match is looked for in tiers. Every tier is tried, and the
// best-scoring one that is worth showing wins — a tier that scores higher
// and is not worth showing loses to one that is, which is what keeps a
// coincidence from hiding the real match behind it:
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
//      target's consecutive words, each target word opening with the
//      query's once the ending the query's would drop is allowed for. So
//      `clone_getter` and `cloned_getter` both reach `cloning_getter`,
//      and so does `clone_g`, where the reader is one letter into a word
//      they have not finished typing. Only the letters two words share
//      are scored, and a mark spans a run of separators but never a run
//      of letters, so a phrase reads as the phrase while `clone` stops
//      short of `cloning`'s `ing`.
//   4. Scattered — the query's characters occur in order with anything at
//      all between them. Names only, per the section above.
//
// Every tier below the first is weaker than it by construction rather than
// by a penalty: each scores only the characters it actually placed,
// against what those characters would have earned had the query been
// there whole. The tail of a word the target ends differently, and a
// separator the target does not spell the same way, are characters the
// reader typed that earn nothing. So a near miss cannot reach what an
// exact match earns.
//
// What a tier counts as the query differs, because what it is able to
// place does. Verbatim and respaced place every character the query
// holds, separators and all, and are charged for all of them. The
// variants tier never matches a target character against a separator — a
// separator is how the reader marks where one word ends — so it is
// charged for the query's words alone. Charging it for the delimiters
// too would mean a reader who has typed `cloned ` and is about to type
// `getter` scoring below one who stopped at `cloned`, which is a result
// vanishing halfway through being typed.
//
// ---- Worth showing, and how well it matched -------------------------------
//
// Those are two questions, and this file keeps them apart. How well a
// match scores orders the results against each other: only the
// differences matter, and the numbers above are expected to be re-tuned.
// Whether the target is worth showing at all is a different question, and
// answering it with a bar on the same number ties the two together — a
// re-tuning then moves what a reader can find, and a query one character
// longer can push a result back over a bar it had fallen under, so it
// leaves the list and returns. `admits` therefore reads no score:
//
//   * The reader's first characters have to land where they aimed: at the
//     opening of a word, or at the end of one they typed most of, which
//     is how `error` finds `thiserror_usage` without an `e` that merely
//     falls last in `single` finding anything.
//   * After that, one run may begin inside a word and no more. One is a
//     slip of the fingers, and keeping it is what still finds
//     `excessive_nesting` for `excessive_nestng`. Two is the characters
//     falling where they may, which is how `bare` would otherwise answer
//     `needless_borrowed_parameters`.
//
// Both are properties of where the match landed, so neither moves when a
// weight does, and typing one more character cannot turn a rejection back
// into a result by arithmetic. Over every lint name typed out whole, run
// together, spaced, abbreviated to initials, misspelt and spelt as a
// variant, one target in ten thousand leaves the list and comes back.
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
      // The target's word has to open with the query's, once the ending
      // the query's word would drop is allowed for. Equal stems are the
      // case this started from — `cloned` against `cloning` — and the
      // looser test is what also takes a word still being typed, where
      // the reader is three letters into `getter` and the stem of what
      // they have so far is `get`.
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
    // The characters this tier is able to place: the query's words, not
    // the delimiters between them. A delimiter is how the reader marks
    // where one word ends, and no target character is ever matched
    // against it here, so charging the query for it would mean a reader
    // who has typed `cloned ` and is about to type `getter` scoring
    // worse than one who stopped at `cloned`. The verbatim and respaced
    // tiers do charge for it, and must: there a separator is content,
    // matched against the target's own or missing from it.
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
        best = better(best, alignWords(parts, stems, span, haystack, at));
      }
      at = haystack.indexOf(stems[0], at + 1);
    }
    return best;
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
    // Where the reader's first characters landed is what the match is
    // about. Landing in the middle of a word is a coincidence however the
    // rest of it falls.
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
   * The better of two matches, counting only those worth showing. A tier
   * that scores higher but is not worth showing loses to one that is,
   * which is what keeps a coincidence from hiding the real match behind
   * it: `clone` lands in `cloning_getter` as a scattered `clon` plus a
   * stray `e`, and as the variants tier's `clon`, and the second is the
   * one a reader means.
   * @param {{ score: number, ranges: number[][] } | null} left
   * @param {{ score: number, ranges: number[][] } | null} right
   * @param {string} haystack
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function betterAdmitted(left, right, haystack) {
    if (right && !admits(right.ranges, haystack)) right = null;
    return better(left, right);
  }

  // How much higher a score has to be to count as higher at all. Two
  // tiers that place the same characters in the same places earn the same
  // total, and they reach it by different arithmetic — one character at a
  // time against one multiplication per word — which doubles do not
  // always agree on to the last bit. The tolerance is many orders of
  // magnitude below the smallest difference the scoring can mean, so it
  // can only ever absorb that noise.
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
    // Every tier is tried rather than the first that matches winning: a
    // verbatim match that is not worth showing must not stand in the way
    // of one that is, which it would if finding the query whole ended the
    // search.
    var found = betterAdmitted(null, matchVerbatim(needle, haystack), haystack);
    found = betterAdmitted(found, matchRespaced(needle, haystack), haystack);
    found = betterAdmitted(found, matchVariants(needle, haystack), haystack);
    return betterAdmitted(found, matchScattered(needle, haystack), haystack);
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
    var found = betterAdmitted(null, matchVerbatim(needle, haystack), haystack);
    found = betterAdmitted(found, matchRespaced(needle, haystack), haystack);
    return betterAdmitted(found, matchVariants(needle, haystack), haystack);
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
  };
})();
