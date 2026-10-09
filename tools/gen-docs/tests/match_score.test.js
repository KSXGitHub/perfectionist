// ============================================================================
// match_score.js: what a match earns.
//
// These numbers order results against each other and nothing else, so they
// are expected to be re-tuned. What is pinned here is therefore the
// relationships between them and the one arithmetic identity that has to
// hold whatever they become — not the figures themselves.
// ============================================================================

;(function () {
  var t = perfectionistTests
  var s = perfectionistMatchScore

  t.group('match_score.test.js')

  t.add('where a run opens is worth more than that it opens at all', function () {
    var atStart = s.opening('bare url', 0)
    var atWord = s.opening('bare url', 5)
    var insideWord = s.opening('bare url', 2)
    t.greater(atStart, atWord, 'opening the target beats opening a word inside it')
    t.greater(atWord, insideWord, 'opening a word beats landing inside one')
    t.equal(insideWord, s.BASE, 'landing inside a word earns the base alone')
  })

  t.add('a perfect match scores exactly one', function () {
    // `idealScore` is the divisor that makes two queries' scores
    // comparable: what a query of that length earns matched contiguously
    // from the target's first character. So a target matched against
    // itself has to come back at exactly 1, whatever the weights become.
    // Asked through a tier, because feeding `idealScore` back into
    // `blend` would divide it by itself and hold for any divisor at all.
    var targets = ['a', 'url', 'bare url', 'core instead of std', 'x y z']
    for (var i = 0; i < targets.length; i++) {
      var hit = t.found(perfectionistMatchTiers.matchVerbatim(targets[i], targets[i]), 'a target matches itself')
      t.equal(hit.score, 1, '`' + targets[i] + '` scores 1 against itself')
    }
  })

  t.add('a longer query earns more at best', function () {
    t.greater(s.idealScore(2), s.idealScore(1), 'two characters can earn more than one')
    t.greater(s.idealScore(8), s.idealScore(7), 'and the ideal keeps climbing')
  })

  t.add('a query outrunning its target still scores a fraction', function () {
    // A separator the target spells differently is a character the query
    // carries and the target does not, so the query can be longer than
    // what it matched. Coverage is held to a fraction for that case.
    var score = s.blend(s.idealScore(8), 8, 4)
    t.equal(score, 1, 'coverage is clamped, so the score reaches 1 and stops')
    t.equal(s.QUALITY_WEIGHT + s.COVERAGE_WEIGHT, 1, 'the weights sum to 1')
  })

  t.add("the higher score wins, and a hair's difference is a tie", function () {
    var low = { score: 0.5, ranges: [] }
    var high = { score: 0.9, ranges: [] }
    t.equal(s.better(low, high), high, 'the higher-scoring match wins')
    t.equal(s.better(high, low), high, 'whichever side it arrives on')
    var hair = { score: 0.5 + s.SCORE_EPSILON / 2, ranges: [] }
    t.equal(s.better(low, hair), low, 'a difference under the epsilon leaves the first')
    t.greater(1, s.SCORE_EPSILON, 'and the epsilon is far below any score')
  })

  t.add('either side may be absent', function () {
    var only = { score: 0.5, ranges: [] }
    t.equal(s.better(null, only), only, 'the present one wins against nothing')
    t.equal(s.better(only, null), only, 'on either side')
    t.equal(s.better(null, null), null, 'and nothing beats nothing')
  })
})()
