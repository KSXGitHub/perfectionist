// ============================================================================
// rank.js: the order results come back in, and how many there are.
//
// Where two of a rule's three fields are compared, the fixtures give them
// the *same* text, so the only thing separating the two scores is the
// weight the field carries — which is what the case is about.
// ============================================================================
;(function () {
  var t = perfectionistTests
  var r = perfectionistRank
  var entry = perfectionistRankFixtures.entry
  var only = perfectionistRankFixtures.only

  t.group('rank_ordering.test.js')

  t.add('a name match outranks the same text in a statement', function () {
    var ranked = r.rank(
      [entry({ statement: 'bare_url', order: 0 }), entry({ name: 'bare_url', order: 1 })],
      'bare url',
    )
    t.equal(ranked.length, 2, 'both are results')
    t.deepEqual(ranked[0].nameRanges, [[0, 8]], 'the one whose name matched comes first')
    t.greater(ranked[0].score, ranked[1].score, 'and it scores higher, order notwithstanding')
  })

  t.add('a statement match outranks the same text in a paragraph', function () {
    var ranked = r.rank(
      [
        entry({ paragraphs: ['bare_url'], order: 0 }),
        entry({ statement: 'bare_url', order: 1 }),
      ],
      'bare url',
    )
    t.equal(ranked.length, 2, 'both are results')
    t.equal(ranked[0].entry.order, 1, 'the statement match comes first, despite being second on the page')
    t.greater(ranked[0].score, ranked[1].score, 'a statement is worth more than body prose')
  })

  t.add('a rule matching in two fields scores by the better of them', function () {
    // The three field scores are weighed against each other, not summed:
    // a second, weaker match cannot push a rule past one whose name
    // matched just as well, and no score may leave 0..1.
    var both = only(
      [entry({ name: 'bare_url', statement: 'a bare URL in a comment' })],
      'bare url',
    )
    var nameOnly = only([entry({ name: 'bare_url' })], 'bare url')
    t.ok(both.score <= 1, 'a score is still a fraction')
    t.equal(both.score, nameOnly.score, 'the statement match adds nothing to the better one')
  })

  t.add("ties go to the page's own order", function () {
    // Two rules indistinguishable to the matcher. Engines older than
    // ES2019 don't promise a stable sort, so the order has to be part of
    // the comparison rather than left to the sort.
    var ranked = r.rank(
      [entry({ name: 'bare_url', order: 7 }), entry({ name: 'bare_url', order: 3 })],
      'bare url',
    )
    t.equal(ranked[0].entry.order, 3, 'the one that comes first on the page comes first here')
    t.equal(ranked[1].entry.order, 7, 'and the other after it')
  })

  t.add('the list is capped, and keeps the best of what it drops', function () {
    /** @type {Entry[]} */
    var entries = []
    for (var i = 0; i < 15; i++) {
      entries.push(entry({ name: 'bare_url', order: i }))
    }
    var ranked = r.rank(entries, 'bare url')
    t.greater(entries.length, ranked.length, 'more rules match than the list shows')
    for (var j = 0; j < ranked.length; j++) {
      t.equal(ranked[j].entry.order, j, "what it keeps is the page's own first few")
    }
  })
})()
