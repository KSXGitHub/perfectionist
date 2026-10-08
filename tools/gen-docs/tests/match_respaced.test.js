// ============================================================================
// match.js: the query found with its separators somewhere else.
//
// One separator the query wrote may be missing from the
// target, one the target carries may be absent from the query, and a run of
// either may stand where a single one does.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var m = perfectionistMatch;

  t.group("match_respaced.test.js");

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
    // one per word. Which wins must not come down to the last bit of a
    // double, or the reader sees the phrase marked whole for one query
    // and word by word for the next.
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
    t.greater(exact.score, padded.score, "and win over too many of them as well");
    // The two misspellings are not ordered against each other, and
    // nothing is owed between them: one is the three words with no
    // delimiter and the other the three words with several, the target
    // spells neither, and which tier answers which is an implementation
    // detail. What they owe is to stay results at all.
    // That both are matches at all is what `t.found` above has already
    // asserted; neither is weighed against a bound, because there is
    // none.
  });
})();
