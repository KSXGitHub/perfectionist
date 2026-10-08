// ============================================================================
// match.js: the query found with its words in any order.
//
// A lint name is a handful of words with no sentence to
// put them in order, so a reader half-remembering one types the words that
// come to mind in the order they come. Every word of the query has to be a
// word of the target, each a different one, and the target may carry words
// the query left out.
// ============================================================================
;(function () {
  var t = perfectionistTests
  var m = perfectionistMatch

  t.group('match_reordered.test.js')

  t.add('a name is found by its words in any order', function () {
    var name = 'core_instead_of_std'
    t.ok(m.matchFuzzy('core std instead', name), 'the words, reordered')
    t.ok(m.matchFuzzy('std core instead', name), 'and reordered again')
    t.ok(m.matchFuzzy('instead of core std', name), 'all four of them, reordered')
  })

  t.add('the target may carry words the query left out', function () {
    // Nobody types `of`.
    var hit = t.found(m.matchFuzzy('std core', 'core_instead_of_std'), 'two of the four')
    t.deepEqual(
      hit.ranges,
      [
        [0, 4],
        [16, 19],
      ],
      'marked where the target reads them, not where the query typed them',
    )
  })

  t.add('each word of the query needs a word of its own', function () {
    // Otherwise one `core` in the target would answer any number of them
    // in the query.
    t.equal(m.matchFuzzy('core core', 'core_instead_of_std'), null, 'one word cannot hold two')
    t.equal(m.matchFuzzy('core zzz', 'core_instead_of_std'), null, 'and every word needs one')
  })

  t.add("a word has to open one of the target's, not sit inside it", function () {
    // `stea` is in `instead`, three letters along. Opening a word is what
    // a reader aims at; landing inside one is where letters fall.
    t.equal(m.matchFuzzy('stea core', 'core_instead_of_std'), null, 'inside a word is nothing')
    t.ok(m.matchFuzzy('instead core', 'core_instead_of_std'), 'opening one is a match')
  })

  t.add("a word reaches the target's variants here too", function () {
    // The same reading as the tier above it: the target's word has to
    // open with the query's once the ending the query's would drop is
    // allowed for, so a plural still finds its singular out of order.
    t.ok(m.matchFuzzy('std cores', 'core_instead_of_std'), '`cores` reaches `core`')
  })

  t.add('how the query spaces these words does not change what they earn', function () {
    // Nothing is matched against a separator here either, so the query's
    // own are not charged for — the same rule as the tier above, and the
    // same reason.
    var one = t.found(m.matchFuzzy('std core', 'core_instead_of_std'), 'one space')
    var three = t.found(m.matchFuzzy('std   core', 'core_instead_of_std'), 'three spaces')
    t.equal(one.score, three.score, 'a delimiter is not content, so it costs nothing')
  })

  t.add('the order the name writes them in still scores highest', function () {
    var name = 'core_instead_of_std'
    var written = t.found(m.matchFuzzy('core instead of std', name), 'as written').score
    var reordered = t.found(m.matchFuzzy('core std instead', name), 'reordered').score
    var fewer = t.found(m.matchFuzzy('std core', name), 'two of them, reordered').score
    t.greater(written, reordered, 'its own order beats any other')
    t.greater(reordered, fewer, 'and more of its words beats fewer')
  })
})()
