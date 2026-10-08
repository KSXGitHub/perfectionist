// ============================================================================
// match.js: what a query reaches at all.
//
// Two questions sit behind that, and a score answers neither.
//
// Which tiers a matcher runs: `matchFuzzy` lets the query come apart, its
// characters scattered and its words reordered, where `matchPhrase` lets
// neither.
//
// And which of the matches found are worth showing: decided by where the
// match landed, never by what it scored, so re-tuning a weight cannot move
// what a reader is able to find.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var m = perfectionistMatch;

  t.group("match_worth_showing.test.js");

  t.add("matchPhrase keeps the query's words whole", function () {
    // It will meet a separator spelled differently and a word ended
    // differently, but never let the letters inside a word drift apart,
    // which is the one thing `matchFuzzy` allows and it does not.
    t.ok(m.matchPhrase("url", "bare_url"), "`url` appears in `bare_url`");
    // `bur` and not `brl`: `admits` turns `brl` away under either
    // matcher, so it would hold without `matchPhrase` running fewer tiers
    // at all. `matchFuzzy` takes `bur`, which isolates the one tier.
    t.equal(m.matchPhrase("bur", "bare_url"), null, "`bur` is `bare_url` scattered");
    t.equal(m.matchPhrase("zzz", "bare_url"), null, "`zzz` does not appear at all");
  });

  t.add("matchFuzzy falls back to scattered characters", function () {
    t.ok(m.matchFuzzy("bur", "bare_url"), "`b` and `ur` open the two words in order");
    t.equal(m.matchFuzzy("lrb", "bare_url"), null, "out of order is not a match");
    t.equal(m.matchFuzzy("zzz", "bare_url"), null, "absent characters are not a match");
  });

  t.add("prose does not take its words in any order", function () {
    // A paragraph a few hundred characters long carries almost any two
    // words somewhere apart from each other, so this freedom would answer
    // nearly every rule on the page. Names are short enough to afford it.
    t.ok(m.matchFuzzy("std core", "core instead of std"), "a name does");
    t.equal(m.matchPhrase("std core", "core instead of std"), null, "prose does not");
  });

  t.add("a real match is one and a coincidence is not", function () {
    // `bare` sits whole in one name and is strewn through another that
    // has nothing to do with it, landing twice inside words.
    t.ok(m.matchFuzzy("bare", "bare_url"), "a real match is a match");
    t.equal(
      m.matchFuzzy("bare", "needless_borrowed_parameters"),
      null,
      "and a coincidence is nothing at all",
    );
  });

  t.add("the first characters have to land where the reader aimed", function () {
    // Landing in the middle of a word is a coincidence however the rest
    // of the match falls: `letter` reaches `cloning_getter` by the `l` of
    // `cloning` and the whole of `getter`, and scores well doing it.
    t.equal(m.matchFuzzy("letter", "cloning_getter"), null, "a mid-word start is not a match");
    t.ok(m.matchFuzzy("letter", "single_letter_generic"), "the same query opening a word is");
  });

  t.add("a word an identifier ran together is aimed, a last letter is not", function () {
    // `error` is the back five of `thiserror`'s nine, so a reader who
    // types it has typed one of the words that name ran together.
    t.ok(m.matchFuzzy("error", "thiserror_usage"), "most of a word is aimed");
    // The `e` that `single` ends in is a letter that happens to fall
    // last, and the reader skipped five to reach it.
    t.equal(m.matchPhrase("e", "single_letter_generic"), null, "one letter of six is not");
    // Where the line falls: `error` is five of `thiserror`'s nine and
    // `rror` four, so the first is more of the word than was skipped and
    // the second is not.
    t.equal(m.matchPhrase("rror", "thiserror"), null, "four of nine is not most of it");
    t.ok(m.matchPhrase("error", "thiserror"), "five of nine is");
    // And the tie the line is drawn on: `ter` is the back three of
    // `letter`'s six, holding exactly as much of the word as it skipped.
    t.ok(m.matchPhrase("ter", "single_letter_generic"), "three of six is as much as was skipped");
  });

  t.add("one run may begin inside a word and no more", function () {
    // A dropped letter leaves the rest of its word trailing off a word
    // opening. One of those is a slip of the fingers and the reader still
    // meant the name; two is the characters falling where they may.
    t.ok(
      m.matchFuzzy("excessive_nestng", "excessive_nesting"),
      "a dropped letter is still the name",
    );
    t.equal(m.matchFuzzy("brl", "bare_url"), null, "two slips are a coincidence");
  });

  t.add("an earlier coincidence does not cost a name its real match", function () {
    // `nic` sits inside `unicode`, where the reader aimed nothing, and
    // finishes `panic`, where they did. The two score alike, so a tier
    // choosing between its own placements by score alone kept the first
    // and the rule was not offered at all.
    var hit = t.found(
      m.matchFuzzy("nic", "unicode_ellipsis_in_panic_messages"),
      "`nic` finds the rule",
    );
    t.deepEqual(hit.ranges, [[22, 25]], "marked where `panic` ends, not inside `unicode`");
  });

  t.add("a match once made is not lost to the next keystroke", function () {
    // Typing `cloned_getter` out passes through `cloned ` and `cloned g`,
    // where little of the query has landed yet. The score may dip — it
    // is an ordering, and orderings move — but the rule must not leave
    // the list and come back, which is the one thing a reader reads as
    // the search being broken.
    var full = "cloned_getter";
    var seen = false;
    for (var i = 1; i <= full.length; i++) {
      var typed = full.slice(0, i);
      var shown = !!m.matchFuzzy(typed, "cloning_getter");
      if (seen) t.ok(shown, JSON.stringify(typed) + " still finds cloning_getter");
      seen = seen || shown;
    }
    t.ok(seen, "and it was found somewhere along the way");
  });

  t.add("what is worth showing does not move when a weight does", function () {
    // The scoring exists to order results and is expected to be re-tuned;
    // what a reader can find must not follow it about. So the library
    // exports no number a caller could weigh a match against.
    var offered = [];
    for (var key in m) offered.push(key);
    offered.sort();
    t.deepEqual(offered, ["excerpt", "matchFuzzy", "matchPhrase"], "no bound is exported");
  });
})();
