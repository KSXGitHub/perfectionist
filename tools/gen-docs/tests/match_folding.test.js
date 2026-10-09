// ============================================================================
// match.js: how the two strings are read before anything is matched.
//
// Folding decides what "the same" means for every tier: a query
// and a target are compared with case ignored, and `_`, `-` and a space
// read alike, because one lint goes by all three spellings. An empty query
// or target belongs here too, as the edge of the same reading.
// ============================================================================

;(function () {
  var t = perfectionistTests
  var m = perfectionistMatch

  t.group('match_folding.test.js')

  t.add('an empty query or target matches nothing', function () {
    t.equal(m.matchFuzzy('', 'bare_url'), null, 'an empty query matches nothing')
    t.equal(m.matchFuzzy('bare', ''), null, 'an empty target matches nothing')
    t.equal(m.matchPhrase('', 'bare_url'), null, 'an empty query matches nothing')
    t.equal(m.matchPhrase('bare', ''), null, 'an empty target matches nothing')
  })

  t.add('case is ignored', function () {
    t.deepEqual(
      m.matchFuzzy('BARE', 'bare_url'),
      m.matchFuzzy('bare', 'bare_url'),
      'an upper-case query scores as its lower-case self'
    )
    t.ok(m.matchPhrase('url', 'BARE_URL'), 'an upper-case target matches too')
  })

  t.add('`_`, `-` and a space are the same character', function () {
    // One lint goes by all three spellings: `bare_url` in an attribute,
    // `bare-url` in its page fragment, "bare URL" in its statement.
    var underscore = m.matchFuzzy('bare_url', 'bare_url')
    t.deepEqual(m.matchFuzzy('bare-url', 'bare_url'), underscore, '`-` reads as `_`')
    t.deepEqual(m.matchFuzzy('bare url', 'bare_url'), underscore, 'a space reads as `_`')
    t.ok(m.matchPhrase('bare url', 'bare_url'), 'and verbatim matching folds them too')
  })

  t.add('a range indexes the string as it was handed in', function () {
    // Folding is one character in, one character out, which every range
    // rests on: `\u0130` lower-cases to two code units, and one of those
    // ahead of a match would shift the range one along from what matched.
    var target = '\u0130stanbul_rule'
    var hit = t.found(m.matchFuzzy('stanbul', target), '`stanbul` is in there')
    var marked = t.found(hit.ranges[0], 'the match is marked')
    t.equal(target.slice(marked[0], marked[1]), 'stanbul', 'the range covers what matched')
  })

  t.add('a run of separators folds to a run of spaces, not to one', function () {
    // Each separator becomes one space rather than a run becoming one,
    // so the folded string indexes exactly like the string handed in —
    // which is what lets a range be used to highlight that string.
    // Collapsing would shift every index after the run.
    var target = 'a  bare  url'
    var hit = t.found(m.matchPhrase('bare', target), '`bare` appears')
    var phrase = t.found(hit.ranges[0], 'the phrase is marked')
    t.equal(
      target.slice(phrase[0], phrase[1]),
      'bare',
      'the range still cuts the word out of the string as it was given'
    )
  })
})()
