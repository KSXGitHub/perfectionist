// ============================================================================
// The fixtures the rank cases are built from, loaded before them the way
// the harness is.
//
// Every fixture is a rule in the shape `rank` wants, never a rule off the
// page: the scraping that produces them is search_overlay.js's and is not
// under test here.
//
// They are their own script because the cases that use them are spread
// over several files, and a helper copied into each would be a copy per
// file to keep saying the same thing. Both runners load this the way they load
// the harness — before any case file, since a case reads it as it is
// evaluated.
// ============================================================================

var perfectionistRankFixtures = (function () {
  var t = perfectionistTests;
  var r = perfectionistRank;

  /**
   * One entry. The defaults match no query any case types, so a fixture
   * names only the field it is about.
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
   * yields exactly one. Most cases are about a single rule, and a second
   * result would make every assertion below it read the wrong row.
   * @param {Entry[]} entries
   * @param {string} query
   * @returns {Result}
   */
  function only(entries, query) {
    var ranked = r.rank(entries, query);
    t.equal(ranked.length, 1, "`" + query + "` should find exactly one of these");
    return ranked[0];
  }

  return {
    entry: entry,
    only: only,
  };
})();
