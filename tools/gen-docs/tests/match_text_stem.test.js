// ============================================================================
// match_text.js: reducing a word to the stem it shares with its variants.
//
// The variants tier reaches `cloning_getter` from `cloned_getter` because
// both words stem alike, so the stemmer decides what counts as a variant.
// Over-stemming would let unrelated words match; under-stemming loses the
// plural a reader typed.
// ============================================================================
;(function () {
  var t = perfectionistTests
  var x = perfectionistMatchText

  t.group('match_text_stem.test.js')

  t.add('one family, one stem', function () {
    var stem = x.stem('cloning')
    t.equal(x.stem('cloned'), stem, '`cloned` stems as `cloning` does')
    t.equal(x.stem('clone'), stem, 'and so does `clone`')
  })

  t.add('a plural reaches its singular', function () {
    t.equal(x.stem('urls'), x.stem('url'), '`urls` and `url` stem alike')
    t.equal(x.stem('boxes'), x.stem('box'), 'and so do `boxes` and `box`')
  })

  t.add('a short word is left whole', function () {
    // Stemming a short word leaves too little to tell words apart, so a
    // word under `MIN_STEM_WORD` keeps every character.
    t.greater(x.MIN_STEM_WORD, x.MIN_STEM, 'a word must outrun the stem it would leave')
    t.equal(x.stem('cl'), 'cl', 'a two-letter word is its own stem')
    t.equal(x.stem('is'), 'is', 'nothing is taken off it')
  })

  t.add('a consonant doubled for an ending comes off with it', function () {
    // English doubles a final consonant before `-ed` or `-ing`, so the
    // doubling has to go when the ending does, or `stopped` would stem to
    // `stopp` and never meet `stop`.
    t.equal(x.stem('stopped'), x.stem('stop'), '`stopped` and `stop` stem alike')
    t.equal(x.undouble('stopp'), 'stop', 'the doubling comes off the bare stem')
    t.equal(x.undouble('fall'), 'fall', "but a word's own double stays")
    t.equal(x.undouble('running'), 'running', 'and nothing is taken before the ending is')
  })

  t.add('an ending is recognised only at the end', function () {
    t.equal(x.endsWith('cloning', 'ing'), true, '`cloning` ends with `ing`')
    t.equal(x.endsWith('ingot', 'ing'), false, '`ingot` only begins with it')
    t.equal(x.endsWith('ing', 'cloning'), false, 'a suffix longer than the word fits nowhere')
  })
})()
