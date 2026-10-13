// ==========================================================================
// Text handling for query matching: folding a string to the form a match
// is looked for in, splitting it into words, and reducing a word to the
// stem it shares with its variants.
// ==========================================================================

/**
 * Where a word begins and ends: a half-open character range, as
 * `String#slice` takes them.
 * @typedef {readonly [start: number, end: number]} Span
 */

var perfectionistMatchText = (function () {
  /**
   * Case-fold a string and flatten `_`, `-` and a space to one another,
   * because one lint goes by all three spellings: `bare_url` is
   * `bare-url` in its page fragment and "bare URL" in its statement.
   *
   * One character in, one character out, because every range handed back
   * indexes the folded string and is read against the original. Lower-
   * casing is not length-preserving — `İ` comes back as two code units —
   * so where it would grow the string the case is left alone. Such a
   * target then misses a query in another case, which beats a highlight
   * on text the reader never typed.
   * @param {string} text
   * @returns {string}
   */
  function fold(text) {
    var lowered = text.toLowerCase()
    return (lowered.length === text.length ? lowered : text).replace(/[-_\s]/g, ' ')
  }

  /**
   * Is the character at `index` the start of a word? The haystack is
   * folded by the time this runs, so upper case needs no test and every
   * separator reads as the space it became.
   * @param {string} haystack
   * @param {number} index
   * @returns {boolean}
   */
  function isWordStart(haystack, index) {
    if (index <= 0) {
      return true
    }
    var previous = haystack.charAt(index - 1)
    return !(previous >= 'a' && previous <= 'z') && !(previous >= '0' && previous <= '9')
  }

  // ---- Words ------------------------------------------------------------
  //
  // A word is a run of letters and digits, so a separator or the
  // punctuation a paragraph carries bounds one without belonging to it.

  /**
   * @param {string} ch
   * @returns {boolean}
   */
  function isAlnum(ch) {
    return (ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9')
  }

  /**
   * @param {string} text
   * @param {number} from
   * @returns {number}
   */
  function wordEnd(text, from) {
    var at = from
    while (at < text.length && isAlnum(text.charAt(at))) {
      at++
    }
    return at
  }

  /**
   * @param {string} text
   * @param {number} from
   * @returns {number}
   */
  function nextWord(text, from) {
    var at = from
    while (at < text.length && !isAlnum(text.charAt(at))) {
      at++
    }
    return at < text.length ? at : -1
  }

  /**
   * @param {string} text
   * @returns {readonly Span[]}
   */
  function wordSpans(text) {
    /** @type {Span[]} */
    var out = []
    var at = nextWord(text, 0)
    while (at >= 0) {
      var end = wordEnd(text, at)
      out.push([at, end])
      at = nextWord(text, end)
    }
    return out
  }

  /**
   * @param {string} text
   * @returns {readonly string[]}
   */
  function words(text) {
    var spans = wordSpans(text)
    /** @type {string[]} */
    var out = []
    for (var i = 0; i < spans.length; i++) {
      out.push(text.slice(spans[i][0], spans[i][1]))
    }
    return out
  }

  /**
   * @param {string} text
   * @param {number} from
   * @param {number} to
   * @returns {boolean}
   */
  function onlySeparators(text, from, to) {
    for (var at = from; at < to; at++) {
      if (text.charAt(at) !== ' ') {
        return false
      }
    }
    return true
  }

  /**
   * @param {string} left
   * @param {string} right
   * @returns {number}
   */
  function commonPrefix(left, right) {
    var limit = Math.min(left.length, right.length)
    var at = 0
    while (at < limit && left.charAt(at) === right.charAt(at)) {
      at++
    }
    return at
  }

  // ---- Stems ------------------------------------------------------------

  // The shortest word a suffix comes off, and the shortest stem left
  // behind. `bed` is too short to take an ending off at all, and `doing`
  // would leave too little of itself to tell from another word.
  var MIN_STEM_WORD = 4

  var MIN_STEM = 3

  /**
   * Does `text` end with `suffix`? Spelled out rather than
   * `String.prototype.endsWith`, which the page's oldest engines predate.
   * @param {string} text
   * @param {string} suffix
   * @returns {boolean}
   */
  function endsWith(text, suffix) {
    var at = text.length - suffix.length
    return at >= 0 && text.indexOf(suffix, at) === at
  }

  /**
   * Drop one of a doubled final consonant, which is what English put
   * there when the ending went on: `getting` leaves `gett`, and the word
   * behind it is `get`. An `ll`, `ss` or `zz` is the word's own doubling
   * — `fall`, `pass` — and stays.
   * @param {string} word
   * @returns {string}
   */
  function undouble(word) {
    if (word.length - 1 < MIN_STEM) {
      return word
    }
    var last = word.charAt(word.length - 1)
    if (last !== word.charAt(word.length - 2)) {
      return word
    }
    if (last === 'l' || last === 's' || last === 'z') {
      return word
    }
    return word.slice(0, word.length - 1)
  }

  /**
   * The stem a word shares with its variants: `cloning`, `cloned`,
   * `clones` and `clone` all come back `clon`, so a reader who types one
   * finds a lint named with another. The regular endings and nothing more
   * — no dictionary, and no ending that rewrote the word rather than
   * extending it, so `getter` stems to itself. Characters come off the end
   * only, so a stem is always a prefix of its word, which `matchVariants`
   * is built on.
   * @param {string} word
   * @returns {string}
   */
  function stem(word) {
    if (word.length < MIN_STEM_WORD) {
      return word
    }
    var out = word
    // A plural or a third person. An `ss` or a `us` is neither: `pass`
    // and `status` end that way on their own account.
    if (endsWith(out, 's') && !endsWith(out, 'ss') && !endsWith(out, 'us') && out.length - 1 >= MIN_STEM) {
      out = out.slice(0, out.length - 1)
    }
    if (endsWith(out, 'ing') && out.length - 3 >= MIN_STEM) {
      out = undouble(out.slice(0, out.length - 3))
    } else if (endsWith(out, 'ed') && out.length - 2 >= MIN_STEM) {
      out = undouble(out.slice(0, out.length - 2))
    }
    // The `e` an `-ing` or an `-ed` form drops anyway, so `clone` is met
    // where `cloning` and `cloned` already are.
    if (endsWith(out, 'e') && out.length - 1 >= MIN_STEM) {
      out = out.slice(0, out.length - 1)
    }
    return out
  }

  return {
    fold: fold,
    isWordStart: isWordStart,
    isAlnum: isAlnum,
    wordEnd: wordEnd,
    nextWord: nextWord,
    wordSpans: wordSpans,
    words: words,
    onlySeparators: onlySeparators,
    commonPrefix: commonPrefix,
    MIN_STEM_WORD: MIN_STEM_WORD,
    MIN_STEM: MIN_STEM,
    endsWith: endsWith,
    undouble: undouble,
    stem: stem,
  }
})()
