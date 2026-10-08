// ============================================================================
// match_text.js: reading a string as words.
//
// Where a word begins and ends decides what the word-wise tiers can
// place, and what any tier earns for where its runs opened. Asked of the
// scanner directly, because a tier that disagreed with it would still
// find something — just not the thing the reader aimed at.
// ============================================================================

(function () {
  var t = perfectionistTests;
  var x = perfectionistMatchText;

  t.group("match_text_words.test.js");

  t.add("a span per word, separators excluded", function () {
    t.deepEqual(
      x.wordSpans("bare url x"),
      [
        [0, 4],
        [5, 8],
        [9, 10],
      ],
      "one span per word, the single-letter word included",
    );
    t.deepEqual(x.wordSpans(""), [], "no words in an empty string");
    t.deepEqual(x.wordSpans("   "), [], "no words in separators alone");
  });

  t.add("a run of separators parts two words, not three", function () {
    t.deepEqual(
      x.words("bare   url"),
      ["bare", "url"],
      "a run of separators yields no empty word between them",
    );
    t.deepEqual(x.words(" bare "), ["bare"], "nor do leading and trailing ones");
  });

  t.add("the first character opens a word", function () {
    // `charAt(-1)` is `""`, which is not a letter, so index 0 would read as
    // a word start even without the guard. Pinned because the opening of
    // the target is what scores highest, and a scanner that said otherwise
    // would quietly cost every exact match its head start.
    t.equal(x.isWordStart("bare url", 0), true, "index 0 opens a word");
    t.equal(x.isWordStart("bare url", 5), true, "so does a character after a separator");
    t.equal(x.isWordStart("bare url", 2), false, "a character inside a word does not");
  });

  t.add("a digit continues a word; a separator does not", function () {
    t.equal(x.isAlnum("a"), true, "a letter is part of a word");
    t.equal(x.isAlnum("1"), true, "so is a digit, so `utf8` is one word");
    t.equal(x.isAlnum("_"), false, "a separator is not");
    // The character *after* the digit, so the digit is what has to deny
    // it: with a letter in front the assertion passes on the letter alone
    // and the digit goes unchecked.
    t.equal(x.isWordStart("utf8x", 4), false, "a character after a digit does not");
    t.equal(x.isWordStart("utf8_x", 5), true, "one after a separator does");
  });

  t.add("a word ends where the separators begin", function () {
    t.equal(x.wordEnd("bare url", 0), 4, "the first word ends at the separator");
    t.equal(x.wordEnd("bare url", 5), 8, "the last ends at the string's end");
    t.equal(x.nextWord("bare url", 4), 5, "the next word opens after the separator");
    t.equal(x.nextWord("bare   url", 4), 7, "however many separators stand between");
  });

  t.add("a stretch of separators is told from one holding a letter", function () {
    t.equal(x.onlySeparators("a  b", 1, 3), true, "two spaces are separators alone");
    t.equal(x.onlySeparators("a b c", 1, 4), false, "a letter between them is not");
  });

  t.add("two words share a prefix up to their first difference", function () {
    t.equal(x.commonPrefix("clone", "cloning"), 4, "`clon` is shared, `e` and `i` differ");
    t.equal(x.commonPrefix("bare", "bare"), 4, "a word shares all of itself");
    t.equal(x.commonPrefix("bare", "url"), 0, "nothing is shared from the first character");
  });
})();
