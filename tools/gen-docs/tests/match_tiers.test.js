// ============================================================================
// match_tiers.js: each tier asked on its own.
//
// Through `matchFuzzy` the tiers cover for each other — every one is tried
// and the best worth showing wins, so a tier that stopped working can go
// unnoticed because a sibling places the same characters. Each is asked
// directly here, on strings already folded, which is what the tiers take.
// ============================================================================

;(function () {
  var t = perfectionistTests
  var r = perfectionistMatchTiers

  t.group('match_tiers.test.js')

  t.add('verbatim turns an empty needle away rather than looping', function () {
    // `indexOf("")` clamps a `fromIndex` past the end instead of returning
    // -1, so the scan over every occurrence would never leave its loop.
    // The entry points reject an empty query before any tier sees one, so
    // this guard is against a hang, not a wrong answer — which is why it
    // is asked of the tier rather than through `matchFuzzy`.
    t.equal(r.matchVerbatim('', 'bare url'), null, 'an empty needle matches nothing')
    t.equal(r.matchVerbatim('', ''), null, 'an empty haystack too')
  })

  t.add('verbatim takes the whole needle, or nothing', function () {
    t.deepEqual(
      t.found(r.matchVerbatim('bare', 'bare url'), '`bare` is in `bare url`').ranges,
      [[0, 4]],
      'the range covers the needle where it stands'
    )
    t.equal(r.matchVerbatim('bareurl', 'bare url'), null, 'a separator missing is not verbatim')
    t.equal(r.matchVerbatim('url bare', 'bare url'), null, 'nor is another order')
  })

  t.add('verbatim prefers the occurrence worth showing', function () {
    // `nic` is inside `unicode` and finishes `panic`. The two score alike,
    // so a scan that kept the first would keep the coincidence.
    var hit = t.found(r.matchVerbatim('nic', 'unicode ellipsis in panic messages'), '`nic` occurs twice in that name')
    t.deepEqual(hit.ranges, [[22, 25]], 'the run finishing `panic` is the one kept')
  })

  t.add('respaced lets the separators disagree', function () {
    t.ok(r.matchRespaced('bareurl', 'bare url'), 'a separator the query omits')
    t.ok(r.matchRespaced('bare   url', 'bare url'), 'a run where the target has one')
    t.ok(r.matchRespaced('this error', 'thiserror usage'), 'one the target omits')
    t.equal(r.matchRespaced('url bare', 'bare url'), null, 'but not another order')
  })

  t.add("variants let a word's ending differ", function () {
    t.ok(r.matchVariants('clone getter', 'cloning getter'), 'a different ending')
    t.ok(r.matchVariants('urls', 'bare url'), 'a plural reaching its singular')
    t.ok(r.matchVariants('clone g', 'cloning getter'), 'a word still being typed')
    t.equal(r.matchVariants('url bare', 'bare url'), null, 'but each word still needs the next target word')
  })

  t.add('reordered lets the words arrive in any order', function () {
    t.ok(r.matchReordered('url bare', 'bare url'), 'the words reversed')
    t.ok(r.matchReordered('core std instead', 'core instead of std'), 'and a word passed by')
    t.equal(
      r.matchReordered('bare url missing', 'bare url'),
      null,
      'but every query word needs a target word of its own'
    )
  })

  t.add('scattered lets the characters come apart', function () {
    var hit = t.found(r.matchScattered('bur', 'bare url'), '`bur` is scattered through it')
    t.greater(hit.ranges.length, 1, 'the characters land in more than one run')
    t.equal(r.matchScattered('zq', 'bare url'), null, 'characters the target lacks match nothing')
  })

  t.add('a tighter tier scores higher than a looser one', function () {
    // Which is why every tier is tried and the best taken, rather than the
    // first that answers.
    var exact = t.found(r.matchVerbatim('bare url', 'bare url'), 'the name itself')
    var respaced = t.found(r.matchRespaced('bareurl', 'bare url'), 'its separators dropped')
    var scattered = t.found(r.matchScattered('brl', 'bare url'), 'its letters alone')
    t.greater(exact.score, respaced.score, 'the name itself beats a respacing of it')
    t.greater(respaced.score, scattered.score, 'and a respacing beats scattered letters')
  })
})()
