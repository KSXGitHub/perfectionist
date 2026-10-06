// ============================================================================
// rank.js: which rules a query finds, in what order, and which of a rule's
// text a result shows.
//
// Every fixture below is a rule in the shape `rank` wants, never a rule off
// the page: the scraping that produces them is search_overlay.js's and is
// not under test here. Where two of a rule's three fields are compared, the
// fixtures give them the *same* text, so the only thing separating the two
// scores is the weight the field carries — which is what the case is about.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var r = perfectionistRank;

  t.group("rank.test.js");

  /**
   * One entry. The defaults match no query any case here types, so a
   * fixture names only the field it is about.
   * @param {Partial<Entry>} fields
   * @returns {Entry}
   */
  function entry(fields) {
    return {
      name: fields.name === undefined ? "unrelated_name" : fields.name,
      href: fields.href === undefined ? "#unrelated-name" : fields.href,
      statement: fields.statement === undefined ? "nothing of interest" : fields.statement,
      paragraphs: fields.paragraphs === undefined ? [] : fields.paragraphs,
      order: fields.order === undefined ? 0 : fields.order,
    };
  }

  /**
   * The one result `entries` yields for `query`, having asserted that it
   * yields exactly one. Most cases here are about a single rule, and a
   * second result would make every assertion below it read the wrong row.
   * @param {Entry[]} entries
   * @param {string} query
   * @returns {Result}
   */
  function only(entries, query) {
    var ranked = r.rank(entries, query);
    t.equal(ranked.length, 1, "`" + query + "` should find exactly one of these");
    return ranked[0];
  }

  // ---- Reading a rule's text ----------------------------------------------

  t.add("flatten makes one line of a wrapped paragraph", function () {
    // The page holds the rendered markdown's line breaks and indentation,
    // and a query is typed as one sentence.
    t.equal(r.flatten("  a\n    wrapped\tline  "), "a wrapped line", "and trims the ends");
    t.equal(r.flatten(""), "", "an empty paragraph stays empty");
  });

  t.add("prose drops an example's pseudo-heading", function () {
    // `Avoid:` and `Prefer:` open an example's paragraphs and say nothing
    // about which rule the reader wants; the rest of the paragraph does.
    t.equal(r.prose("Avoid:\n  the one thing"), "the one thing", "`Avoid:` goes");
    t.equal(r.prose("Prefer: the other"), "the other", "`Prefer:` goes");
    t.equal(
      r.prose("something to Avoid: this"),
      "something to Avoid: this",
      "only an opening one goes — mid-paragraph it is prose",
    );
  });

  // ---- What a query finds -------------------------------------------------

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

  t.add("a name match shows the statement, unmarked", function () {
    // Nothing of the statement matched, so there is nothing to highlight
    // in it — but it is still the line that says what the rule does.
    var hit = only([entry({ name: "bare_url" })], "bare url");
    t.equal(hit.text, "nothing of interest", "the statement shows regardless");
    t.deepEqual(hit.textRanges, [], "with no marks in it");
  });

  t.add("the best-matching paragraph is the one shown", function () {
    var hit = only(
      [
        entry({
          paragraphs: [
            "a bare URL mentioned once in a long rambling run of words that dilutes its coverage",
            "a bare URL",
          ],
        }),
      ],
      "bare url",
    );
    t.equal(hit.text, "a bare URL", "the tighter of the two paragraphs");
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

  // ---- How the three fields rank against each other -----------------------

  t.add("a name match outranks the same text in a statement", function () {
    var ranked = r.rank(
      [entry({ statement: "bare_url", order: 0 }), entry({ name: "bare_url", order: 1 })],
      "bare url",
    );
    t.equal(ranked.length, 2, "both are results");
    t.deepEqual(ranked[0].nameRanges, [[0, 8]], "the one whose name matched comes first");
    t.greater(ranked[0].score, ranked[1].score, "and it scores higher, order notwithstanding");
  });

  t.add("a statement match outranks the same text in a paragraph", function () {
    var ranked = r.rank(
      [
        entry({ paragraphs: ["bare_url"], order: 0 }),
        entry({ statement: "bare_url", order: 1 }),
      ],
      "bare url",
    );
    t.equal(ranked.length, 2, "both are results");
    t.equal(ranked[0].entry.order, 1, "the statement match comes first, despite being second on the page");
    t.greater(ranked[0].score, ranked[1].score, "a statement is worth more than body prose");
  });

  // ---- Ordering and the list's length -------------------------------------

  t.add("ties go to the page's own order", function () {
    // Two rules indistinguishable to the matcher. Engines older than
    // ES2019 don't promise a stable sort, so the order has to be part of
    // the comparison rather than left to the sort.
    var ranked = r.rank(
      [entry({ name: "bare_url", order: 7 }), entry({ name: "bare_url", order: 3 })],
      "bare url",
    );
    t.equal(ranked[0].entry.order, 3, "the one that comes first on the page comes first here");
    t.equal(ranked[1].entry.order, 7, "and the other after it");
  });

  t.add("the list is capped, and keeps the best of what it drops", function () {
    /** @type {Entry[]} */
    var entries = [];
    for (var i = 0; i < 15; i++) {
      entries.push(entry({ name: "bare_url", order: i }));
    }
    var ranked = r.rank(entries, "bare url");
    t.greater(entries.length, ranked.length, "more rules match than the list shows");
    for (var j = 0; j < ranked.length; j++) {
      t.equal(ranked[j].entry.order, j, "what it keeps is the page's own first few");
    }
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

  t.add("a rule matching in two fields scores by the better of them", function () {
    // The three field scores are weighed against each other, not summed:
    // a second, weaker match cannot push a rule past one whose name
    // matched just as well, and no score may leave 0..1.
    var both = only(
      [entry({ name: "bare_url", statement: "a bare URL in a comment" })],
      "bare url",
    );
    var nameOnly = only([entry({ name: "bare_url" })], "bare url");
    t.ok(both.score <= 1, "a score is still a fraction");
    t.equal(both.score, nameOnly.score, "the statement match adds nothing to the better one");
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

  // ---- The invariant the renderer depends on ------------------------------

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
