// ============================================================================
// match_admit.js: whether a match is worth showing.
//
// It reads no score at all. Asked of `admits` and `aimed` directly:
// through a tier, a
// coincidence that one tier turns away can still arrive from another, so
// the rule itself would go unchecked.
// ============================================================================
;(function () {
  var t = perfectionistTests
  var a = perfectionistMatchAdmit

  t.group('match_admit.test.js')

  t.add('the first run has to land where the reader aimed', function () {
    t.equal(a.aimed('bare url', [0, 4]), true, 'a run opening a word was aimed')
    t.equal(a.aimed('bare url', [5, 8]), true, 'at the opening of any word')
    t.equal(a.aimed('bare url', [1, 3]), false, 'a run starting inside one was not')
  })

  t.add('a run finishing a word the reader typed most of was aimed', function () {
    // How `error` finds `thiserror_usage`: the run ends where the word
    // does, so the reader typed the end of a word rather than landing in
    // the middle of one.
    t.equal(a.aimed('thiserror usage', [4, 9]), true, 'a run ending a word was aimed')
    t.equal(a.aimed('single', [5, 6]), false, 'a last character alone was not')
  })

  t.add('admission turns on where the runs landed, not what they scored', function () {
    t.equal(a.admits([[0, 4]], 'bare url'), true, 'a run opening the target is shown')
    t.equal(a.admits([[1, 3]], 'bare url'), false, 'one starting inside a word is not')
  })

  t.add('one run may begin inside a word, and no more', function () {
    // One is a slip of the fingers; two is the characters falling where
    // they may.
    t.equal(a.SLIPS_ALLOWED, 1, 'one slip is allowed')
    t.equal(
      a.admits(
        [
          [0, 1],
          [6, 7],
        ],
        'bare url getter',
      ),
      true,
      'one run inside a word is a slip',
    )
    t.equal(
      a.admits(
        [
          [0, 1],
          [6, 7],
          [11, 12],
        ],
        'bare url getter',
      ),
      false,
      'two is the characters falling where they may',
    )
  })

  t.add('a placement worth showing beats one that is not', function () {
    // Within a tier as well as between tiers: a tier that kept only its
    // highest-scoring placement would drop the target when that placement
    // was a coincidence and a lower-scoring one was the real match.
    var coincidence = { score: 0.9, ranges: [[1, 3]] }
    var aimedAt = { score: 0.5, ranges: [[0, 4]] }
    t.equal(
      a.betterAdmitted(coincidence, aimedAt, 'bare url'),
      aimedAt,
      'the shown placement wins though it scores lower',
    )
    t.equal(
      a.betterAdmitted(aimedAt, coincidence, 'bare url'),
      aimedAt,
      'whichever side it arrives on',
    )
  })

  t.add('between two worth showing, the score decides', function () {
    var low = { score: 0.5, ranges: [[0, 4]] }
    var high = { score: 0.9, ranges: [[5, 8]] }
    t.equal(a.betterAdmitted(low, high, 'bare url'), high, 'the higher-scoring one wins')
    t.equal(a.betterAdmitted(null, low, 'bare url'), low, 'and either side may be absent')
  })
})()
