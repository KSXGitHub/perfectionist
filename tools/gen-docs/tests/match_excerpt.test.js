// ============================================================================
// match.js: windowing a long text around what matched.
//
// `excerpt` is the one thing the library offers that matches nothing. A
// rule's prose paragraph runs to several hundred characters and a list of
// those is unreadable, so a result shows a window around the match — with
// the ranges shifted onto it, and the ones that fall outside dropped rather
// than left pointing past its end.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var m = perfectionistMatch;

  t.group("match_excerpt.test.js");

  t.add("a short text is left alone", function () {
    var ranges = [[4, 9]];
    var kept = m.excerpt("the quick brown fox", ranges, 100);
    t.equal(kept.text, "the quick brown fox", "nothing is cut");
    t.deepEqual(kept.ranges, ranges, "and nothing moves");
  });

  t.add("a long text is windowed around the match, ranges and all", function () {
    var text = "the quick brown fox jumps over the lazy dog";
    var windowed = m.excerpt(text, [[20, 25]], 20);
    t.greater(text.length, windowed.text.length, "the text is cut down");
    // Asserted before the range is read, so a window that lost it fails
    // on this claim rather than crashing on the next line.
    t.equal(windowed.ranges.length, 1, "the match is inside the window the match anchored");
    t.equal(
      windowed.text.slice(windowed.ranges[0][0], windowed.ranges[0][1]),
      "jumps",
      "the shifted range still covers the word that matched",
    );
    // A quarter of the window is kept ahead of the match, so the reader
    // sees what it sits in rather than meeting it against the ellipsis.
    t.greater(windowed.ranges[0][0], 1, "the window opens before the match, not on it");
  });

  t.add("a match near either end still fills the window", function () {
    var text = "the quick brown fox jumps over the lazy dog";
    // The lead-in would run the window past the text, so it is clamped to
    // the end — which is what keeps a window this side of the text as
    // long as one in the middle of it.
    var tail = m.excerpt(text, [[40, 43]], 20);
    t.greater(tail.text.length, 20, "the window is as long as it would be anywhere else");
    t.equal(
      tail.text.slice(tail.ranges[0][0], tail.ranges[0][1]),
      "dog",
      "and the range still covers the word that matched",
    );
  });

  t.add("a match longer than the window is marked through it", function () {
    var text = "the quick brown fox jumps over the lazy dog";
    // The whole text matched, which is more than the window holds. Every
    // character the reader can see is still part of what they typed, so
    // the range comes back cut to the window rather than dropped for not
    // fitting inside it.
    var windowed = m.excerpt(text, [[0, text.length]], 20);
    t.equal(windowed.ranges.length, 1, "the range survives the window");
    t.equal(
      windowed.text.slice(windowed.ranges[0][0], windowed.ranges[0][1]),
      "the quick brown fox ",
      "cut to what the window shows of it",
    );
  });

  t.add("a window that cuts either end says so", function () {
    var text = "the quick brown fox jumps over the lazy dog";
    t.equal(m.excerpt(text, [[20, 25]], 20).text.indexOf("…"), 0, "a cut head is marked");
    t.equal(m.excerpt(text, [[4, 9]], 20).text.slice(-1), "…", "a cut tail is marked");
  });

  t.add("a range outside the window is dropped, not left dangling", function () {
    // Shifted onto the window, an index from outside it lands outside the
    // string — and highlight.js slices by it.
    var text = "the quick brown fox jumps over the lazy dog";
    var windowed = m.excerpt(text, [[40, 43], [4, 9]], 12);
    t.equal(windowed.ranges.length, 1, "the range that fell outside the window is gone");
    t.equal(
      windowed.text.slice(windowed.ranges[0][0], windowed.ranges[0][1]),
      "dog",
      "and the one that survived covers what it covered before",
    );
  });
})();
