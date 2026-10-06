// ============================================================================
// match.js: scoring a query against a string, and windowing a long string
// around what matched.
//
// The cases below assert orderings and ranges rather than exact scores
// wherever they can. An exact score pins the arithmetic, which is the part
// most likely to be deliberately changed — the weights were chosen by eye
// and are expected to be re-tuned — whereas "a contiguous match beats a
// scattered one" is the property that must survive any re-tuning. Where a
// number is asserted it is a threshold relationship, not a literal.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var m = perfectionistMatch;

  t.group("match.test.js");

  // ---- What counts as a match ---------------------------------------------

  t.add("matchPhrase keeps the query's words whole", function () {
    // It will meet a separator spelled differently and a word ended
    // differently — the two sections further down — but never let the
    // letters inside a word drift apart, which is the one thing
    // `matchFuzzy` allows and it does not.
    t.ok(m.matchPhrase("url", "bare_url"), "`url` appears in `bare_url`");
    t.equal(m.matchPhrase("brl", "bare_url"), null, "`brl` is `bare_url` scattered");
    t.equal(m.matchPhrase("zzz", "bare_url"), null, "`zzz` does not appear at all");
  });

  t.add("matchFuzzy falls back to scattered characters", function () {
    t.ok(m.matchFuzzy("brl", "bare_url"), "`b`, `r`, `l` occur in that order");
    t.equal(m.matchFuzzy("lrb", "bare_url"), null, "out of order is not a match");
    t.equal(m.matchFuzzy("zzz", "bare_url"), null, "absent characters are not a match");
  });

  t.add("an empty query or target matches nothing", function () {
    t.equal(m.matchFuzzy("", "bare_url"), null, "an empty query matches nothing");
    t.equal(m.matchFuzzy("bare", ""), null, "an empty target matches nothing");
    t.equal(m.matchPhrase("", "bare_url"), null, "an empty query matches nothing");
    t.equal(m.matchPhrase("bare", ""), null, "an empty target matches nothing");
  });

  // ---- Folding ------------------------------------------------------------

  t.add("case is ignored", function () {
    t.deepEqual(
      m.matchFuzzy("BARE", "bare_url"),
      m.matchFuzzy("bare", "bare_url"),
      "an upper-case query scores as its lower-case self",
    );
    t.ok(m.matchPhrase("url", "BARE_URL"), "an upper-case target matches too");
  });

  t.add("`_`, `-` and a space are the same character", function () {
    // One lint goes by all three spellings: `bare_url` in an attribute,
    // `bare-url` in its page fragment, "bare URL" in its statement.
    var underscore = m.matchFuzzy("bare_url", "bare_url");
    t.deepEqual(m.matchFuzzy("bare-url", "bare_url"), underscore, "`-` reads as `_`");
    t.deepEqual(m.matchFuzzy("bare url", "bare_url"), underscore, "a space reads as `_`");
    t.ok(m.matchPhrase("bare url", "bare_url"), "and verbatim matching folds them too");
  });

  t.add("a run of separators folds to a run of spaces, not to one", function () {
    // Each separator becomes one space rather than a run becoming one,
    // so the folded string indexes exactly like the string handed in —
    // which is what lets a range be used to highlight that string.
    // Collapsing would shift every index after the run.
    var target = "a  bare  url";
    var hit = t.found(m.matchPhrase("bare", target), "`bare` appears");
    t.equal(
      target.slice(hit.ranges[0][0], hit.ranges[0][1]),
      "bare",
      "the range still cuts the word out of the string as it was given",
    );
  });

  // ---- Which occurrence is taken ------------------------------------------

  t.add("a verbatim match prefers the occurrence that opens a word", function () {
    // `item` occurs twice: inside `bitem` at 1, and opening a word at 6.
    var hit = t.found(m.matchPhrase("item", "bitem_item"), "`item` appears");
    t.deepEqual(hit.ranges, [[6, 10]], "the word-opening occurrence is the one taken");
  });

  t.add("a verbatim match falls back to the first occurrence", function () {
    // No occurrence opens a word, so the earliest stands.
    var hit = t.found(m.matchPhrase("ite", "bitembite"), "`ite` appears");
    t.deepEqual(hit.ranges, [[1, 4]], "the earliest occurrence is the one taken");
  });

  t.add("ranges mark exactly what matched", function () {
    // The highlight is drawn from these, so an off-by-one shows up as
    // highlighted text the reader never typed.
    var whole = t.found(m.matchPhrase("url", "bare_url"), "`url` appears whole");
    t.deepEqual(whole.ranges, [[5, 8]], "one contiguous range");
    // `b` at 0, then `url` contiguous from 5: two runs, not four characters.
    var runs = t.found(m.matchFuzzy("burl", "bare_url"), "`burl` matches scattered");
    t.deepEqual(runs.ranges, [[0, 1], [5, 8]], "one range per run");
  });

  // ---- Ordering -----------------------------------------------------------

  t.add("a contiguous match beats a scattered one", function () {
    var contiguous = t.found(m.matchFuzzy("bare", "bare_url"), "whole").score;
    var scattered = t.found(m.matchFuzzy("bare", "needless_borrowed_parameters"), "strewn").score;
    t.greater(contiguous, scattered, "`bare` sits whole in one and is strewn through the other");
  });

  t.add("a word-opening match beats a mid-word one", function () {
    var opening = t.found(m.matchPhrase("url", "bare_url"), "opens a word").score;
    var midWord = t.found(m.matchPhrase("url", "blurline"), "sits mid-word").score;
    t.greater(opening, midWord, "opening a word is worth more than landing inside one");
  });

  t.add("opening the name beats opening a word inside it", function () {
    // Both of these open a word, so both were once perfect on quality,
    // and the shorter one won on coverage alone — which put the name that
    // merely contains the letter above the one that starts with it.
    t.greater(
      t.found(m.matchFuzzy("n", "named_prelude_imports"), "starts with `n`").score,
      t.found(m.matchFuzzy("n", "excessive_nesting"), "contains `n`").score,
      "a name that starts with the query comes first",
    );
    // The longer of the two starting with it still comes first.
    t.greater(
      t.found(m.matchFuzzy("n", "needless_borrowed_parameters"), "starts with `n`").score,
      t.found(m.matchFuzzy("n", "excessive_nesting"), "contains `n`").score,
      "length does not buy back the head start",
    );
  });

  t.add("a name the query merely opens a word in is still a match", function () {
    // The point of the bonus is the order, not an exclusion: a reader who
    // types `n` should still be offered `excessive_nesting`, under the
    // two that begin with one.
    t.greater(
      t.found(m.matchFuzzy("n", "excessive_nesting"), "contains `n`").score,
      m.FILTER_MIN_SCORE,
      "it clears the filter bound",
    );
  });

  t.add("the head start fades as the query grows", function () {
    // The bonus is a fixed amount spread over the whole query, so the
    // lead it buys shrinks as the query grows specific enough to decide
    // itself. One letter is decided by it; six are very nearly not.
    /**
     * How far ahead of the name that merely contains `query` the name
     * that starts with it scores.
     * @param {string} query
     * @returns {number}
     */
    function lead(query) {
      return (
        t.found(m.matchFuzzy(query, "import_grouping_mismatch"), "starts with it").score -
        t.found(m.matchFuzzy(query, "named_prelude_imports"), "contains it").score
      );
    }
    t.greater(lead("i"), 0, "the name that starts with the query leads either way");
    t.greater(lead("i"), lead("import"), "but by less and less as the query gets longer");
  });

  t.add("the shorter target wins a tie", function () {
    var brief = t.found(m.matchFuzzy("bare", "bare_url"), "short target").score;
    var lengthy = t.found(m.matchFuzzy("bare", "bare_identifier_reference"), "long target").score;
    t.greater(brief, lengthy, "both match `bare` perfectly, so length is all that separates them");
  });

  t.add("initials score well against a snake_case name", function () {
    // Typing a word's initials is a thing readers do with identifiers, and
    // it is why `_` earns a word-start bonus.
    t.greater(
      t.found(m.matchFuzzy("bur", "bare_url"), "`bur` matches").score,
      m.FILTER_MIN_SCORE,
      "`bur` should find `bare_url`",
    );
  });

  // ---- The bounds ---------------------------------------------------------

  t.add("a score is a fraction", function () {
    var samples = [
      t.found(m.matchFuzzy("bare_url", "bare_url"), "the whole name"),
      t.found(m.matchFuzzy("b", "bare_url"), "one character"),
      t.found(m.matchFuzzy("brl", "bare_url"), "scattered characters"),
      t.found(m.matchPhrase("are", "bare_url"), "a mid-word phrase"),
    ];
    for (var i = 0; i < samples.length; i++) {
      t.greater(samples[i].score, 0, "a match scores above nothing");
      t.ok(samples[i].score <= 1, "a match scores at most one");
    }
  });

  t.add("the bounds separate a real match from a coincidence", function () {
    // The reason the bounds exist: `bare` happens to occur, strewn, in a
    // name that has nothing to do with it.
    t.greater(
      t.found(m.matchFuzzy("bare", "bare_url"), "a real match").score,
      m.FILTER_MIN_SCORE,
      "a real match clears the filter bound",
    );
    t.greater(
      m.FILTER_MIN_SCORE,
      t.found(m.matchFuzzy("bare", "needless_borrowed_parameters"), "a coincidence").score,
      "a coincidence does not",
    );
    t.greater(m.FILTER_MIN_SCORE, m.SEARCH_MIN_SCORE, "the filter bound is the tighter one");
  });

  // ---- Separators that do not line up -------------------------------------
  //
  // The second tier: the query is there, but its separators are not where
  // the target's are. Each case below is one way that happens, and the
  // last holds the orderings between them.

  t.add("a separator the target does not have is not needed", function () {
    var hit = t.found(
      m.matchPhrase("this error", "thiserror_usage"),
      "`this error` is `thiserror` with a separator the lint does not have",
    );
    t.deepEqual(hit.ranges, [[0, 9]], "the range covers the word it found");
  });

  t.add("a separator the target has need not be typed", function () {
    var target = "Flags closure parameters whose identifier is one letter";
    var hit = t.found(
      m.matchPhrase("flagsclosureparameters", target),
      "a query run together still finds the words it ran together",
    );
    t.equal(
      target.slice(hit.ranges[0][0], hit.ranges[0][1]),
      "Flags closure parameters",
      "the whole phrase is marked, the separators it stepped over included",
    );
  });

  t.add("a run of separators in the query stands for one, or for none", function () {
    t.ok(
      m.matchPhrase("flags  closure   parameters", "Flags closure parameters whose"),
      "too many spaces is still the phrase the target spells with one each",
    );
    t.ok(
      m.matchPhrase("this  error", "thiserror_usage"),
      "and still the word the target spells with none",
    );
  });

  t.add("a separator the target interposes breaks the run", function () {
    // The two targets are the same length, so their coverage is identical
    // and only quality can separate them: the `c` the target's own
    // separator pushed along is paid as a word's opening rather than as a
    // run's continuation. That is the whole of why a respaced match
    // scores below one whose separators line up, so pin it on its own
    // rather than leave it to the comparison below, where the queries
    // differ in length and the shorter-target term could carry the
    // ordering by itself.
    t.equal("abcdx".length, "ab cd".length, "the two targets are the same length");
    var run = t.found(m.matchPhrase("abcd", "abcdx"), "`abcd` opens the first");
    var broken = t.found(m.matchPhrase("abcd", "ab cd"), "and spans the second");
    t.greater(run.score, broken.score, "crossing the separator costs the run");
  });

  t.add("one phrase is marked one way however its separators were typed", function () {
    // The respaced and variants tiers place the same characters here and
    // so score the same, and they mark differently — the one span against
    // one per word. Which was taken used to come down to the last bit of
    // a double, since the two reach the same total by different
    // arithmetic, and the reader saw the phrase marked whole for one
    // query and word by word for the next.
    var target = "Flags closure parameters whose identifier is one letter";
    var joined = t.found(m.matchPhrase("flagsclosureparameters", target), "run together");
    var padded = t.found(m.matchPhrase("flags  closure   parameters", target), "padded");
    t.deepEqual(padded.ranges, joined.ranges, "the phrase is marked as the phrase either way");
  });

  t.add("a query whose separators line up scores above one whose do not", function () {
    var target = "Flags closure parameters whose identifier is one letter";
    var exact = t.found(m.matchPhrase("flags closure parameters", target), "as written");
    var joined = t.found(m.matchPhrase("flagsclosureparameters", target), "run together");
    var padded = t.found(m.matchPhrase("flags  closure   parameters", target), "padded");
    t.greater(exact.score, joined.score, "the separators as the target spells them win");
    t.greater(joined.score, padded.score, "a separator typed and absent costs more than one absent and typed");
  });

  // ---- Words that end differently -----------------------------------------
  //
  // The third tier: the query's words align with the target's one for one,
  // a word matching by the stem it shares with its variants where it does
  // not match in full.

  t.add("a word's ending may differ", function () {
    t.ok(m.matchFuzzy("clone_getter", "cloning_getter"), "`clone` reaches `cloning`");
    t.ok(m.matchFuzzy("cloned_getter", "cloning_getter"), "and so does `cloned`");
    t.ok(m.matchPhrase("cloning a field", "clones a field"), "prose gets the same");
  });

  t.add("a plural finds its singular", function () {
    t.ok(m.matchFuzzy("urls", "bare_url"), "`urls` reaches `bare_url`");
    t.ok(m.matchPhrase("fields", "a struct field"), "and `fields` a field");
  });

  t.add("an ending that rewrote the word is not a variant", function () {
    // Only the regular endings are stripped, so `getting` is `get`
    // doubled rather than `getter` lengthened and the two stay two words.
    // The first word has to be a variant that lands, or the second is
    // never reached and the claim would hold for the wrong reason.
    t.ok(
      m.matchPhrase("clone getting", "cloning getting"),
      "`clone` reaches `cloning`, which puts the word after it in reach",
    );
    t.equal(
      m.matchPhrase("clone getter", "cloning getting"),
      null,
      "and there `getter` does not reach `getting`",
    );
  });

  t.add("the query's words have to be consecutive in the target", function () {
    // Without this the tier would answer any paragraph carrying the same
    // words somewhere apart from each other.
    t.ok(m.matchPhrase("clone getter", "cloning getter"), "one after the other is a match");
    t.equal(
      m.matchPhrase("clone getter", "cloning ref getter"),
      null,
      "a word in between is not",
    );
  });

  t.add("a variant scores below the word itself", function () {
    var itself = t.found(m.matchFuzzy("cloning_getter", "cloning_getter"), "the name");
    var shorter = t.found(m.matchFuzzy("clone_getter", "cloning_getter"), "`clone`");
    var other = t.found(m.matchFuzzy("cloned_getter", "cloning_getter"), "`cloned`");
    t.greater(itself.score, shorter.score, "a variant cannot reach the word it varies from");
    t.greater(shorter.score, other.score, "and the more of the word it shares, the closer it gets");
  });

  t.add("only the letters the two words share are marked", function () {
    var hit = t.found(m.matchFuzzy("clone_getter", "cloning_getter"), "`clone_getter`");
    t.deepEqual(
      hit.ranges,
      [
        [0, 4],
        [8, 14],
      ],
      "`clon` of `cloning` and the whole of `getter`, and not the `ing`",
    );
  });

  // ---- Windowing ----------------------------------------------------------

  t.add("a short text is left alone", function () {
    var ranges = [[4, 9]];
    var kept = m.excerpt("the quick brown fox", ranges, 100);
    t.equal(kept.text, "the quick brown fox", "nothing is cut");
    t.deepEqual(kept.ranges, ranges, "and nothing moves");
  });

  t.add("a long text is windowed around the match, ranges and all", function () {
    var text = "the quick brown fox jumps over the lazy dog";
    var windowed = m.excerpt(text, [[20, 25]], 20);
    t.greater(text.length, windowed.text.length, "the text is cut down");
    // Asserted before the range is read, so a window that lost it fails
    // on this claim rather than crashing on the next line.
    t.equal(windowed.ranges.length, 1, "the match is inside the window the match anchored");
    t.equal(
      windowed.text.slice(windowed.ranges[0][0], windowed.ranges[0][1]),
      "jumps",
      "the shifted range still covers the word that matched",
    );
  });

  t.add("a window that cuts either end says so", function () {
    var text = "the quick brown fox jumps over the lazy dog";
    t.equal(m.excerpt(text, [[20, 25]], 20).text.indexOf("…"), 0, "a cut head is marked");
    t.equal(m.excerpt(text, [[4, 9]], 20).text.slice(-1), "…", "a cut tail is marked");
  });

  t.add("a range outside the window is dropped, not left dangling", function () {
    // Shifted onto the window, an index from outside it lands outside the
    // string — and highlight.js slices by it.
    var text = "the quick brown fox jumps over the lazy dog";
    var windowed = m.excerpt(text, [[40, 43], [4, 9]], 12);
    t.equal(windowed.ranges.length, 1, "the range that fell outside the window is gone");
    t.equal(
      windowed.text.slice(windowed.ranges[0][0], windowed.ranges[0][1]),
      "dog",
      "and the one that survived covers what it covered before",
    );
  });
})();
