// ============================================================================
// rank.js: turning a page's text into what ranking wants.
//
// `flatten` and `prose` run over a rule's paragraphs before any of them is
// matched against anything. Neither ranks, which is why they are here
// rather than among the cases that do.
// ============================================================================

;(function () {
  var t = perfectionistTests
  var r = perfectionistRank

  t.group('rank_text.test.js')

  t.add('flatten makes one line of a wrapped paragraph', function () {
    // The page holds the rendered markdown's line breaks and indentation,
    // and a query is typed as one sentence.
    t.equal(r.flatten('  a\n    wrapped\tline  '), 'a wrapped line', 'and trims the ends')
    t.equal(r.flatten(''), '', 'an empty paragraph stays empty')
  })

  t.add("prose drops an example's pseudo-heading", function () {
    // `Avoid:` and `Prefer:` open an example's paragraphs and say nothing
    // about which rule the reader wants; the rest of the paragraph does.
    t.equal(r.prose('Avoid:\n  the one thing'), 'the one thing', '`Avoid:` goes')
    t.equal(r.prose('Prefer: the other'), 'the other', '`Prefer:` goes')
    t.equal(
      r.prose('something to Avoid: this'),
      'something to Avoid: this',
      'only an opening one goes — mid-paragraph it is prose'
    )
  })
})()
