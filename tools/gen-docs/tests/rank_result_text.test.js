// ============================================================================
// rank.js: which of a rule's text a result shows.
//
// A result carries one run of text beneath the name, and which run that is
// follows from which field the query reached. These cases are about that
// choice, the window a long paragraph is cut to, and the invariant the
// renderer depends on: every range indexes into the text handed back with
// it rather than the text it was cut from.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var r = perfectionistRank;
  var entry = perfectionistRankFixtures.entry;
  var only = perfectionistRankFixtures.only;

  t.group("rank_result_text.test.js");

  t.add("a name match shows the statement, unmarked", function () {
    // Nothing of the statement matched, so there is nothing to highlight
    // in it — but it is still the line that says what the rule does.
    var hit = only([entry({ name: "bare_url" })], "bare url");
    t.equal(hit.text, "nothing of interest", "the statement shows regardless");
    t.deepEqual(hit.textRanges, [], "with no marks in it");
  });

  t.add("the best-matching paragraph is the one shown", function () {
    // The tighter paragraph is listed first, so taking the last that
    // matched would answer the other one — which is what makes this the
    // best rather than merely not the first.
    var hit = only(
      [
        entry({
          paragraphs: [
            "a bare URL",
            "a bare URL mentioned once in a long rambling run of words that dilutes its coverage",
          ],
        }),
      ],
      "bare url",
    );
    t.equal(hit.text, "a bare URL", "the tighter of the two paragraphs");
  });

  t.add("a paragraph shows only where it is the best of the three", function () {
    // The name is what the query matched; the paragraph carries it too,
    // less well. The statement is the line that shows, as it does for
    // any name match — a paragraph displaces it on its own score, not on
    // merely having matched.
    var hit = only(
      [
        entry({
          name: "bare_url",
          statement: "a statement",
          paragraphs: ["something about a bare URL, mentioned among other things"],
        }),
      ],
      "bare_url",
    );
    t.equal(hit.text, "a statement", "the statement, not the weaker paragraph");
  });

  t.add("a long paragraph is cut down, and its marks come with it", function () {
    var padding = "";
    while (padding.length < 400) {
      padding += "words that say nothing in particular ";
    }
    var long = padding + "and then a bare URL at the very end";
    var hit = only([entry({ paragraphs: [long] })], "bare url");
    t.greater(long.length, hit.text.length, "the paragraph does not show whole");
    t.equal(
      hit.text.slice(hit.textRanges[0][0], hit.textRanges[0][1]),
      "bare URL",
      "and the mark still sits on the words that matched",
    );
  });

  t.add("every range indexes into the text handed back with it", function () {
    // highlight.js slices the name and the text by these, so a range past
    // either end silently drops the tail of what the reader can see.
    var ranked = r.rank(
      [
        entry({ name: "bare_url", order: 0 }),
        entry({ statement: "a bare URL in a comment", order: 1 }),
        entry({ paragraphs: ["a bare URL in a comment"], order: 2 }),
      ],
      "bare url",
    );
    t.equal(ranked.length, 3, "all three are results");
    for (var i = 0; i < ranked.length; i++) {
      var hit = ranked[i];
      var bounds = [
        { ranges: hit.nameRanges, extent: hit.entry.name.length },
        { ranges: hit.textRanges, extent: hit.text.length },
      ];
      for (var j = 0; j < bounds.length; j++) {
        for (var k = 0; k < bounds[j].ranges.length; k++) {
          var range = bounds[j].ranges[k];
          t.ok(range[0] >= 0, "a range starts inside the string");
          t.greater(range[1], range[0], "a range is not empty");
          t.ok(range[1] <= bounds[j].extent, "a range ends inside the string");
        }
      }
    }
  });
})();
