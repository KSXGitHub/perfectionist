// ============================================================================
// Ranking rules against a query.
//
// Given the catalogue's rules as plain objects and a query, this returns
// the handful that match, best first. It is where the search's judgement
// lives: which of a rule's three kinds of text a query hit, how those
// three are weighted against each other, which paragraph a result shows,
// and how many results there are.
//
// Nothing here touches the DOM, which is what lets the weights and the
// ordering be exercised outside a browser.
// ============================================================================

/**
 * One rule, in the shape ranking needs: its name, where it lives on the
 * page, its one-line statement, its prose a paragraph at a time, and
 * its position in the page's own order, which breaks ties.
 * @typedef {object} Entry
 * @property {string} name
 * @property {string} href
 * @property {string} statement
 * @property {string[]} paragraphs
 * @property {number} order
 */

/**
 * One ranked result: the rule, the score it ranked by, the matched
 * ranges of its name, and the text to show beneath the name with the
 * matched ranges of *that* text.
 * @typedef {object} Result
 * @property {Entry} entry
 * @property {number} score
 * @property {number[][]} nameRanges
 * @property {string} text
 * @property {number[][]} textRanges
 */

var perfectionistRank = (function () {
  // How the three kinds of match are ranked against each other. A name
  // match scores as itself; the other two are scaled down enough that they
  // can't displace one.
  var NAME_WEIGHT = 1;
  var STATEMENT_WEIGHT = 0.65;
  var TEXT_WEIGHT = 0.4;

  // How many results the list shows.
  var RESULT_LIMIT = 10;

  // The longest run of a paragraph a result shows, in characters. A rule's
  // prose paragraph can run to several hundred, and a result list of those
  // is unreadable; the window is taken around the match.
  var EXCERPT_LIMIT = 180;

  // Stripped off the front of a paragraph that opens with one. These are
  // the example sections' pseudo-headings, not prose.
  var PSEUDO_HEADINGS = ["Avoid:", "Prefer:"];

  /**
   * Collapse the line breaks and indentation the rendered markdown carries
   * into single spaces, so a match can span what reads as one sentence.
   * @param {string} text
   * @returns {string}
   */
  function flatten(text) {
    return text.replace(/\s+/g, " ").trim();
  }

  /**
   * One paragraph or list item's prose: whitespace collapsed, and an
   * `Avoid:` / `Prefer:` pseudo-heading stripped off the front. Those two
   * open an example's paragraphs and say nothing about which rule the
   * reader wants, but the rest of the paragraph that carries one is real
   * prose, so only the opening goes.
   * @param {string} raw  the block's text, as the page holds it
   * @returns {string}
   */
  function prose(raw) {
    var text = flatten(raw);
    for (var i = 0; i < PSEUDO_HEADINGS.length; i++) {
      if (text.indexOf(PSEUDO_HEADINGS[i]) !== 0) continue;
      return text.slice(PSEUDO_HEADINGS[i].length).trim();
    }
    return text;
  }

  // Nothing here decides whether a match is worth showing: a hit is one
  // already worth showing, and its score only orders the results against
  // each other.
  var nameHit = perfectionistMatch.matchFuzzy;
  var proseHit = perfectionistMatch.matchPhrase;

  /**
   * The best-matching of a rule's prose paragraphs.
   * @param {string} query
   * @param {string[]} paragraphs
   * @returns {{ score: number, ranges: number[][], text: string } | null}
   */
  function bestParagraph(query, paragraphs) {
    /** @type {{ score: number, ranges: number[][], text: string } | null} */
    var best = null;
    for (var i = 0; i < paragraphs.length; i++) {
      var found = proseHit(query, paragraphs[i]);
      if (!found) continue;
      if (best && best.score >= found.score) continue;
      best = { score: found.score, ranges: found.ranges, text: paragraphs[i] };
    }
    return best;
  }

  /**
   * Rank `entries` against `query`, best first, capped at
   * `RESULT_LIMIT`.
   * @param {Entry[]} entries
   * @param {string} query
   * @returns {Result[]}
   */
  function rank(entries, query) {
    /** @type {Result[]} */
    var out = [];
    for (var i = 0; i < entries.length; i++) {
      var entry = entries[i];
      var matchedName = nameHit(query, entry.name);
      var statementHit = proseHit(query, entry.statement);
      var paragraphHit = bestParagraph(query, entry.paragraphs);
      var nameScore = matchedName ? matchedName.score * NAME_WEIGHT : 0;
      var statementScore = statementHit ? statementHit.score * STATEMENT_WEIGHT : 0;
      var paragraphScore = paragraphHit ? paragraphHit.score * TEXT_WEIGHT : 0;
      var score = Math.max(nameScore, statementScore, paragraphScore);
      if (score <= 0) continue;
      // The text beneath the name is the rule's statement, except where a
      // prose paragraph is what matched — then it is that paragraph,
      // windowed around the match. Either way the ranges handed to the
      // renderer are the ones matched in the text actually shown, so a
      // result highlights what the reader typed wherever they can see it.
      /** @type {{ text: string, ranges: number[][] }} */
      var shown;
      if (paragraphHit && paragraphScore > nameScore && paragraphScore > statementScore) {
        shown = perfectionistMatch.excerpt(
          paragraphHit.text,
          paragraphHit.ranges,
          EXCERPT_LIMIT
        );
      } else {
        shown = perfectionistMatch.excerpt(
          entry.statement,
          statementHit ? statementHit.ranges : [],
          EXCERPT_LIMIT
        );
      }
      out.push({
        entry: entry,
        score: score,
        nameRanges: matchedName ? matchedName.ranges : [],
        text: shown.text,
        textRanges: shown.ranges,
      });
    }
    // The tie-break is explicit: sort stability cannot be assumed on the
    // engines this page targets.
    out.sort(function (left, right) {
      if (right.score !== left.score) return right.score - left.score;
      return left.entry.order - right.entry.order;
    });
    return out.slice(0, RESULT_LIMIT);
  }

  return {
    flatten: flatten,
    prose: prose,
    rank: rank,
  };
})();
