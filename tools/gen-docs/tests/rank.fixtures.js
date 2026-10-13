// ============================================================================
// The fixtures the rank cases are built from.
//
// Every one is a rule in the shape `rank` wants, never a rule scraped off
// the page: that scraping is not under test here.
// ============================================================================

var perfectionistRankFixtures = (function () {
  var t = perfectionistTests
  var r = perfectionistRank

  /**
   * One entry. The defaults match no query any case types, so a fixture
   * names only the field it is about.
   * @param {Partial<Entry>} fields
   * @returns {Entry}
   */
  function entry(fields) {
    return {
      name: fields.name === undefined ? 'unrelated_name' : fields.name,
      href: fields.href === undefined ? '#unrelated-name' : fields.href,
      statement: fields.statement === undefined ? 'nothing of interest' : fields.statement,
      paragraphs: fields.paragraphs === undefined ? [] : fields.paragraphs,
      order: fields.order === undefined ? 0 : fields.order,
    }
  }

  /**
   * The one result `entries` yields for `query`, having asserted that it
   * yields exactly one. Most cases are about a single rule, and a second
   * result would make every assertion below it read the wrong row.
   * @param {readonly Entry[]} entries
   * @param {string} query
   * @returns {Result}
   */
  function only(entries, query) {
    var ranked = r.rank(entries, query)
    t.equal(ranked.length, 1, '`' + query + '` should find exactly one of these')
    return ranked[0]
  }

  return {
    entry: entry,
    only: only,
  }
})()
