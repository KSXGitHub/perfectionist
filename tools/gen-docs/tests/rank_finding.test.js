// ============================================================================
// rank.js: which rules a query finds.
//
// A rule has three kinds of text and a query may reach it through any of
// them. These cases are about whether it is reached at all — by its name,
// by its statement, by one of its paragraphs, or not at all.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var r = perfectionistRank;
  var entry = perfectionistRankFixtures.entry;
  var only = perfectionistRankFixtures.only;

  t.group("rank_finding.test.js");

  t.add("a query that matches nothing finds nothing", function () {
    t.deepEqual(r.rank([entry({ name: "bare_url" })], "zzz"), [], "no result at all");
  });

  t.add("an empty query finds nothing", function () {
    // The overlay shows its prompt instead; see search_overlay.js.
    t.deepEqual(r.rank([entry({ name: "bare_url" })], ""), [], "no result at all");
  });

  t.add("a rule is found by its name, and the name is marked", function () {
    var hit = only([entry({ name: "bare_url" })], "bare url");
    t.equal(hit.entry.name, "bare_url", "the entry comes back whole");
    t.deepEqual(hit.nameRanges, [[0, 8]], "the whole name matched");
  });

  t.add("a rule is found by its statement, and the statement is marked", function () {
    var hit = only([entry({ statement: "a bare URL in a comment" })], "bare url");
    t.equal(hit.text, "a bare URL in a comment", "the statement is what shows");
    t.equal(
      hit.text.slice(hit.textRanges[0][0], hit.textRanges[0][1]),
      "bare URL",
      "and the mark sits on the words that matched, in the casing the page uses",
    );
  });

  t.add("a rule is found by one of its prose paragraphs", function () {
    var hit = only([entry({ paragraphs: ["a bare URL in a comment"] })], "bare url");
    t.equal(hit.text, "a bare URL in a comment", "the paragraph that matched is what shows");
    t.deepEqual(hit.nameRanges, [], "the name matched nothing, so nothing of it is marked");
  });

  t.add("a letter buried mid-word is not a prose match", function () {
    // `matchPhrase` finds a single character almost anywhere, so without
    // the score bound on prose a letter typed into the search would match
    // the statement of nearly every rule on the page.
    t.deepEqual(
      r.rank([entry({ statement: "an example of prose" })], "x"),
      [],
      "`x` sits inside `example` and nowhere else, so it finds nothing",
    );
  });

  t.add("a coincidence is kept out of the list", function () {
    // `bare` occurs, strewn, in a name that has nothing to do with it; the
    // score bound is what stops every rule on the page from matching every
    // query. See match.js for the scoring this leans on.
    var ranked = r.rank(
      [entry({ name: "needless_borrowed_parameters" }), entry({ name: "bare_url", order: 1 })],
      "bare",
    );
    t.equal(ranked.length, 1, "only the rule the reader meant");
    t.equal(ranked[0].entry.name, "bare_url", "and that is the one");
  });
})();
