// ============================================================================
// match.js: which occurrence a match takes, and what it reports.
//
// Where more than one occurrence would do, the verbatim tier prefers one
// worth showing, then the higher-scoring, then the earliest. What comes
// back is
// one range per contiguous run, and the highlight is drawn from those, so
// an off-by-one here shows up on the page as marked text the reader never
// typed.
// ============================================================================
;(function () {
  var t = perfectionistTests
  var m = perfectionistMatch

  t.group('match_ranges.test.js')

  t.add('a verbatim match prefers the occurrence that opens a word', function () {
    // `item_` occurs twice: inside `bitem_` at 1, and opening a word at
    // 6. The query carries the separator so that this tier is the one
    // answering — it alone scores and marks the separators the query
    // typed, where the looser tiers place the letters between them and
    // trim the span to those, which is the `[6, 10]` below.
    var hit = t.found(m.matchPhrase('item_', 'bitem_item_x'), '`item_` appears')
    t.deepEqual(hit.ranges, [[6, 11]], 'the word-opening occurrence is the one taken')
  })

  t.add('a verbatim match falls back to the first occurrence', function () {
    // Neither occurrence opens a word — each finishes one — so the
    // earliest stands.
    var hit = t.found(m.matchPhrase('ite', 'bite_site'), '`ite` appears')
    t.deepEqual(hit.ranges, [[1, 4]], 'the earliest occurrence is the one taken')
    // And an occurrence that opens no word is still one. Typed from the
    // middle of a name, separator and all, so that the fallback is what
    // the answer rests on rather than a looser tier placing the letters.
    var middle = t.found(
      m.matchPhrase('_instead_of_std', 'core_instead_of_std'),
      '`_instead_of_std` appears',
    )
    t.deepEqual(middle.ranges, [[4, 19]], 'the separator it opens with is marked with it')
  })

  t.add('ranges mark exactly what matched', function () {
    // The highlight is drawn from these, so an off-by-one shows up as
    // highlighted text the reader never typed.
    var whole = t.found(m.matchPhrase('url', 'bare_url'), '`url` appears whole')
    t.deepEqual(whole.ranges, [[5, 8]], 'one contiguous range')
    // `b` at 0, then `url` contiguous from 5: two runs, not four characters.
    var runs = t.found(m.matchFuzzy('burl', 'bare_url'), '`burl` matches scattered')
    t.deepEqual(runs.ranges, [[0, 1], [5, 8]], 'one range per run')
  })

  t.add('initials find a snake_case name', function () {
    // Typing a word's initials is a thing readers do with identifiers, and
    // it is why `_` earns a word-start bonus.
    var hit = t.found(m.matchFuzzy('bur', 'bare_url'), '`bur` finds `bare_url`')
    t.deepEqual(
      hit.ranges,
      [
        [0, 1],
        [5, 7],
      ],
      '`b` opens one word and `ur` the other',
    )
  })
})()
