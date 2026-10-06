// ============================================================================
// match.js: how matches are ordered against each other.
//
// Every case here asserts a relationship rather than a number. The weights
// behind these orderings were chosen by eye and are expected to be
// re-tuned; what has to survive any re-tuning is that a contiguous match
// beats a scattered one, that opening a word beats landing inside one, and
// that opening the target beats opening a word within it.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var m = perfectionistMatch;

  t.group("match_ordering.test.js");

  t.add("a score is a fraction", function () {
    var samples = [
      t.found(m.matchFuzzy("bare_url", "bare_url"), "the whole name"),
      t.found(m.matchFuzzy("b", "bare_url"), "one character"),
      t.found(m.matchFuzzy("bur", "bare_url"), "scattered characters"),
      t.found(m.matchPhrase("are", "bare_url"), "a mid-word phrase"),
    ];
    for (var i = 0; i < samples.length; i++) {
      t.greater(samples[i].score, 0, "a match scores above nothing");
      t.ok(samples[i].score <= 1, "a match scores at most one");
    }
  });

  t.add("a contiguous match beats a scattered one", function () {
    // One query, two names it reaches: in the first its three characters
    // open the two words, in the second they are spread over three.
    var tight = t.found(m.matchFuzzy("bur", "bare_url"), "two words opened").score;
    var strewn = t.found(m.matchFuzzy("bur", "bare_issue_reference"), "spread out").score;
    t.greater(tight, strewn, "characters closer together are worth more");
  });

  t.add("a word-opening match beats a mid-word one", function () {
    var opening = t.found(m.matchPhrase("url", "bare_url"), "opens a word").score;
    var midWord = t.found(m.matchPhrase("url", "curl"), "finishes one").score;
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
    t.ok(m.matchFuzzy("n", "excessive_nesting"), "it is still offered");
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
})();
