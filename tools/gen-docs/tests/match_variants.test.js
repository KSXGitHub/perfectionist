// ============================================================================
// match.js: the query found with its words ending differently.
//
// The third tier: the query's words align one for one with a run of the
// target's consecutive words, each target word opening with the query's
// once the ending the query's word would drop is allowed for. That is what
// carries a plural to its singular, and a word still being typed to the
// whole of it.
//
// Some cases ask `matchPhrase` as well as `matchFuzzy`. Prose runs this
// tier and neither of the two looser ones, so asking it there is what makes
// this tier answer alone rather than lean on one of those to cover for it.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var m = perfectionistMatch;

  t.group("match_variants.test.js");

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

  t.add("a word still being typed is still a match", function () {
    // The reader is partway through the last word, which is every
    // keystroke but the final one. Wanting the word finished before the
    // tier would look at it is what used to hold `cloning_getter` back
    // until `clone_getter` was complete.
    var tail = "getter";
    for (var i = 1; i <= tail.length; i++) {
      var query = "clone " + tail.slice(0, i);
      t.ok(m.matchFuzzy(query, "cloning_getter"), query + " finds the lint");
      // Asked of prose as well, which has neither the reordered tier nor
      // the scattered one: this tier has to take a half-typed word by
      // itself, not lean on a looser one to cover for it.
      t.ok(m.matchPhrase(query, "cloning getter"), query + " finds it in prose too");
    }
  });

  t.add("how the query spaces its words does not change what they earn", function () {
    // This tier never matches a target character against a separator, so
    // the query's own are not charged for — which is what keeps a reader
    // who has typed `cloned ` from scoring below one who stopped at
    // `cloned`. Two spellings of the same two words therefore score alike
    // where a tier charged for them would separate the spellings.
    // Asked of prose, where this tier answers alone: in a name the
    // reordered tier would answer alongside it and could cover for it.
    var one = t.found(m.matchPhrase("clone getter", "cloning getter"), "one space");
    var three = t.found(m.matchPhrase("clone   getter", "cloning getter"), "three spaces");
    t.equal(one.score, three.score, "a delimiter is not content, so it costs nothing");
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
})();
