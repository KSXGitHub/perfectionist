// ============================================================================
// The Search overlay: a modal box over the page that ranks whole rules
// against a query, rather than narrowing one list of names the way the
// filter boxes do.
//
// The magnifier button beside the settings gear opens it, Escape and the
// ✕ Close button dismiss it, and each result is a link to `#/rule/<name>`
// — the same fragment the index table and the sidebar point at, so a
// result lands the reader on the rule article itself.
//
// Everything but the button is built here. The Rust template emits the
// button `hidden` and this file clears that as its last act, the same
// "reveal only once functional" contract the rest of the page's controls
// follow (the `[hidden] { display: none !important }` reset in
// style/base.css keeps `hidden` authoritative). Reading match.js's
// threshold into `SEARCH_MIN_SCORE` below is part of it: if match.js never
// ran, that read throws here, long before the reveal.
//
// ---- Where the searchable text comes from ---------------------------------
//
// The catalogue is one document holding every rule's prose, so the search
// reads the page it is on instead of shipping a copy of that prose as a
// second payload. Nothing can drift, nothing is downloaded twice, and the
// scrape costs nothing until the reader first opens the overlay, at which
// point it runs once and is kept.
//
// The text that comes out of each `article.rule` is of these kinds, and
// the ranking weights them in this order:
//
//   1. the lint name, from the heading;
//   2. the rule's statement — its one-line description, the paragraph
//      carrying the default-state badge;
//   3. its prose, one entry per paragraph and list item.
//
// A weaker match on a name therefore still outranks a perfect match in
// prose, which is what a reader scanning for a half-remembered lint wants.
//
// The name is matched loosely and the other two verbatim, for the reason
// match.js's own header gives: a reader types a name from memory, but types
// words out of prose.
//
// Text that every rule repeats is left out of the scrape, because matching
// it tells the reader nothing about which rule they want. The section
// headings ("What it does", "Why restrict this?", "Example",
// "Configuration", ...) are excluded by construction — only paragraphs and
// list items are collected — and the repeats that remain are excluded by
// name: the "Source:" line, the "Configuration: none." line, and the
// "Configure via dylint.toml under [...]" line inside a Configuration
// panel. The `Avoid:` / `Prefer:` pseudo-headings that open an example's
// paragraphs are stripped off the front of the paragraph that carries
// them, since the rest of that paragraph is real prose.
// ============================================================================

(function () {
  // Also the load-bearing check that match.js ran: see the file header.
  var SEARCH_MIN_SCORE = perfectionistMatch.SEARCH_MIN_SCORE;

  // How the three kinds of match are ranked against each other. A name
  // match scores as itself; the other two are scaled down enough that they
  // can't displace one.
  var NAME_WEIGHT = 1;
  var STATEMENT_WEIGHT = 0.65;
  var TEXT_WEIGHT = 0.4;

  // How many results the list shows. A starting point open to tuning once
  // there is a feel for it, not a considered limit.
  var RESULT_LIMIT = 10;

  // The longest run of a paragraph a result shows, in characters. A rule's
  // prose paragraph can run to several hundred, and a result list of those
  // is unreadable; the window is taken around the match (see
  // `perfectionistMatch.excerpt`).
  var EXCERPT_LIMIT = 180;

  // Stripped off the front of a paragraph that opens with one. These are
  // the example sections' pseudo-headings, not prose.
  var PSEUDO_HEADINGS = ["Avoid:", "Prefer:"];

  var toggle = /** @type {HTMLElement} */ (document.querySelector(".search-toggle"));
  if (!toggle) return;

  // ---- The overlay's markup ---------------------------------------------

  var overlay = document.createElement("div");
  overlay.className = "search-overlay";
  overlay.id = "search-overlay";
  overlay.hidden = true;

  var dialog = document.createElement("div");
  dialog.className = "search-dialog";
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-label", "Search lints");

  // The search box: the full width of the dialog, and its fixed-height
  // part. The results list below takes whatever height is left.
  var box = document.createElement("div");
  box.className = "search-box";

  var input = document.createElement("input");
  input.className = "search-input";
  // `type="search"` earns the platform's search affordances: a clear
  // button on desktop, a dedicated key on phone and tablet keyboards.
  input.type = "search";
  input.placeholder = "Search lints\u2026";
  input.setAttribute("aria-label", "Search lints");
  // Phone and tablet keyboards otherwise capitalise the first letter and
  // autocorrect a half-typed lint name into a dictionary word.
  input.setAttribute("autocapitalize", "none");
  input.setAttribute("autocorrect", "off");
  input.setAttribute("autocomplete", "off");
  input.setAttribute("enterkeyhint", "search");
  input.spellcheck = false;

  var close = document.createElement("button");
  close.type = "button";
  close.className = "search-close";
  // The ✕ is decoration; "Close" is the label, so the glyph is hidden from
  // assistive tech and the button's accessible name stays the word alone.
  var closeGlyph = document.createElement("span");
  closeGlyph.className = "search-close-glyph";
  closeGlyph.setAttribute("aria-hidden", "true");
  closeGlyph.appendChild(document.createTextNode("\u2715"));
  close.appendChild(closeGlyph);
  close.appendChild(document.createTextNode("Close"));

  var results = document.createElement("ul");
  results.className = "search-results";
  results.setAttribute("aria-label", "Search results");

  box.appendChild(input);
  box.appendChild(close);
  dialog.appendChild(box);
  dialog.appendChild(results);
  overlay.appendChild(dialog);
  document.body.appendChild(overlay);
  // Only now that the overlay exists does the button control anything.
  toggle.setAttribute("aria-controls", overlay.id);

  // ---- Scraping the page ------------------------------------------------

  /**
   * One rule, in the shape the ranking needs.
   * @typedef {object} Entry
   * @property {string} name
   * @property {string} href
   * @property {string} statement
   * @property {string[]} paragraphs
   * @property {number} order
   */

  /** @type {Entry[] | null} */
  var entries = null;

  /**
   * The text of `parent`, with one child node left out.
   * @param {Element} parent
   * @param {Node | null} omit
   * @returns {string}
   */
  function textWithout(parent, omit) {
    var out = "";
    for (var i = 0; i < parent.childNodes.length; i++) {
      var node = parent.childNodes[i];
      if (node === omit) continue;
      out += node.textContent || "";
    }
    return out;
  }

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
   * Does every rule repeat this block verbatim? See the file header for
   * which blocks qualify and why they are left out.
   * @param {HTMLElement} block
   * @returns {boolean}
   */
  function isRepeated(block) {
    if (block.classList.contains("source")) return true;
    if (block.classList.contains("config-none")) return true;
    var parent = block.parentElement;
    return !!parent && parent.matches("details.config-details");
  }

  /**
   * The prose of one paragraph or list item, with an `Avoid:` / `Prefer:`
   * pseudo-heading stripped off the front.
   * @param {HTMLElement} block
   * @returns {string}
   */
  function prose(block) {
    var text = flatten(block.textContent || "");
    for (var i = 0; i < PSEUDO_HEADINGS.length; i++) {
      if (text.indexOf(PSEUDO_HEADINGS[i]) !== 0) continue;
      return text.slice(PSEUDO_HEADINGS[i].length).trim();
    }
    return text;
  }

  /**
   * Read every rule off the page. Called once, on the first open.
   * @returns {Entry[]}
   */
  function scrape() {
    /** @type {Entry[]} */
    var out = [];
    var articles = /** @type {NodeListOf<HTMLElement>} */ (
      document.querySelectorAll("article.rule")
    );
    for (var i = 0; i < articles.length; i++) {
      var article = articles[i];
      var nameHost = article.querySelector("h2 .lint-name");
      var id = article.getAttribute("id");
      if (!nameHost || !id) continue;
      // The default-state badge marks the statement paragraph; the
      // statement is that paragraph minus the badge's own word.
      var badge = article.querySelector("p .state");
      var statementHost = badge ? badge.parentElement : null;
      /** @type {string[]} */
      var paragraphs = [];
      var blocks = /** @type {NodeListOf<HTMLElement>} */ (
        article.querySelectorAll("p, li")
      );
      for (var j = 0; j < blocks.length; j++) {
        if (blocks[j] === statementHost || isRepeated(blocks[j])) continue;
        var text = prose(blocks[j]);
        if (text) paragraphs.push(text);
      }
      out.push({
        name: flatten(nameHost.textContent || ""),
        href: "#" + id,
        statement: statementHost ? flatten(textWithout(statementHost, badge)) : "",
        paragraphs: paragraphs,
        order: out.length,
      });
    }
    return out;
  }

  // ---- Ranking ----------------------------------------------------------

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

  /**
   * Score `query` against a lint name, or `null` when the match is too
   * weak to count as one.
   * @param {string} query
   * @param {string} name
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function nameHit(query, name) {
    var found = perfectionistMatch.matchFuzzy(query, name);
    if (!found || found.score < SEARCH_MIN_SCORE) return null;
    return found;
  }

  /**
   * Score `query` against a run of prose, or `null` when the match is too
   * weak to count as one.
   * @param {string} query
   * @param {string} text
   * @returns {{ score: number, ranges: number[][] } | null}
   */
  function proseHit(query, text) {
    var found = perfectionistMatch.matchPhrase(query, text);
    if (!found || found.score < SEARCH_MIN_SCORE) return null;
    return found;
  }

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
   * Rank every rule against `query`, best first, capped at
   * `RESULT_LIMIT`.
   * @param {string} query
   * @returns {Result[]}
   */
  function rank(query) {
    if (!entries) entries = scrape();
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
    // Best match first, ties broken by the page's own rule order. The
    // tie-break is explicit rather than left to the sort's stability,
    // which engines older than ES2019 don't guarantee.
    out.sort(function (left, right) {
      if (right.score !== left.score) return right.score - left.score;
      return left.entry.order - right.entry.order;
    });
    return out.slice(0, RESULT_LIMIT);
  }

  // ---- Rendering --------------------------------------------------------

  /** @param {Result} result */
  function renderResult(result) {
    var item = document.createElement("li");
    var link = document.createElement("a");
    link.className = "search-result";
    link.href = result.entry.href;
    // The score the result was ranked by, for whoever is debugging a
    // ranking that reads wrong.
    link.setAttribute("data-score", result.score.toFixed(4));
    var name = document.createElement("code");
    name.className = "search-result-name";
    perfectionistMatch.renderName(name, result.entry.name, result.nameRanges);
    var text = document.createElement("span");
    text.className = "search-result-text";
    perfectionistMatch.renderText(text, result.text, result.textRanges);
    link.appendChild(name);
    link.appendChild(text);
    item.appendChild(link);
    results.appendChild(item);
  }

  function apply() {
    while (results.firstChild) results.removeChild(results.firstChild);
    var query = input.value.trim();
    // An empty query has nothing to rank, and a query nothing matches
    // ranks nothing: either way the list stays empty.
    if (query === "") return;
    var ranked = rank(query);
    for (var i = 0; i < ranked.length; i++) renderResult(ranked[i]);
  }

  // ---- Background inertness --------------------------------------------
  //
  // The overlay is modal, so Tab must not wander into the page behind it
  // and assistive tech must not read it out. `inert` (HTML standard,
  // Baseline 2023) removes a subtree from both in one step; it is applied
  // to every direct child of <body> except the overlay while it is open,
  // mirroring what nav_toggle.js does for the nav drawer. Each side only
  // clears what it set, so the two never undo each other. Browsers too old
  // for `inert` ignore it and fall back to what the page gives for free:
  // the overlay is still dismissible by its ✕ and by Escape.
  /** @type {HTMLElement[]} */
  var inerted = [];

  function setBackgroundInert() {
    inerted = [];
    for (var i = 0; i < document.body.children.length; i++) {
      var child = /** @type {HTMLElement} */ (document.body.children[i]);
      if (child === overlay) continue;
      if (child.inert) continue;
      child.inert = true;
      inerted.push(child);
    }
  }

  function clearBackgroundInert() {
    for (var i = 0; i < inerted.length; i++) inerted[i].inert = false;
    inerted = [];
  }

  // ---- Open / close -----------------------------------------------------

  function openOverlay() {
    overlay.hidden = false;
    toggle.setAttribute("aria-expanded", "true");
    setBackgroundInert();
    input.focus();
  }

  function closeOverlay() {
    overlay.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
    clearBackgroundInert();
    // The button is what the reader came from and what reopens the
    // overlay, so focus goes back to it rather than dropping to <body>.
    toggle.focus({ preventScroll: true });
  }

  function isOpen() {
    return !overlay.hidden;
  }

  toggle.addEventListener("click", function () {
    if (isOpen()) {
      closeOverlay();
    } else {
      openOverlay();
    }
  });

  close.addEventListener("click", closeOverlay);

  // A click on the backdrop — anywhere in the overlay outside the dialog
  // — dismisses it, the conventional gesture for a modal.
  overlay.addEventListener("click", function (event) {
    if (dialog.contains(/** @type {Node | null} */ (event.target))) return;
    closeOverlay();
  });

  // Escape closes the overlay. The default is suppressed only while the
  // overlay is open, so Escape keeps its ordinary meaning everywhere else
  // on the page — including for the settings panel, which runs its own
  // Escape handler.
  document.addEventListener("keydown", function (event) {
    if (event.key !== "Escape") return;
    if (!isOpen()) return;
    event.preventDefault();
    closeOverlay();
  });

  input.addEventListener("input", apply);
  input.addEventListener("search", apply);
  input.addEventListener("keydown", function (event) {
    if (event.key !== "Enter") return;
    // The input is in no form, so Enter submits nothing; suppressing the
    // default only keeps a stray form association from navigating.
    event.preventDefault();
    apply();
  });

  // Following a result closes the overlay so the rule it lands on is
  // visible, and moves focus there so a keyboard reader keeps their place
  // — the link they just activated is inside a now-hidden overlay, and
  // focus would otherwise drop to <body>. Modifier-key clicks and
  // non-primary buttons are "open in a new tab" gestures and must leave
  // this page as it stands. Rule articles aren't focusable by default, so
  // `tabindex="-1"` goes on first.
  results.addEventListener("click", function (event) {
    var link = /** @type {Element} */ (event.target).closest("a");
    if (!link) return;
    if (event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    closeOverlay();
    var hash = /** @type {HTMLAnchorElement} */ (link).hash;
    if (!hash) return;
    // Rule fragments are `#/rule/<name>`, whose `/` characters make them
    // invalid CSS id selectors — `querySelector("#/rule/...")` would
    // throw. `getElementById` matches the literal `id` and accepts them.
    var target = document.getElementById(decodeURIComponent(hash.slice("#".length)));
    if (!target) return;
    target.setAttribute("tabindex", "-1");
    target.focus({ preventScroll: true });
  });

  // Mirror the hamburger's and the gear's Visual Viewport API
  // compensation (see nav_toggle.js): on mobile browsers that anchor
  // `position: fixed` to the layout viewport rather than the visual one,
  // translate the button by the visual viewport's offset so it stays glued
  // to the top of the visible area as the URL bar collapses. The overlay
  // itself is left alone — it is opened from a tap, and the on-screen
  // keyboard that follows shrinks the visual viewport, so pinning it to
  // that would shrink the dialog out from under the reader's fingers.
  if (window.visualViewport) {
    var vv = window.visualViewport;
    var syncToViewport = function () {
      toggle.style.transform = "translate(" + vv.offsetLeft + "px, " + vv.offsetTop + "px)";
    };
    vv.addEventListener("scroll", syncToViewport);
    vv.addEventListener("resize", syncToViewport);
    syncToViewport();
  }

  // Everything is built and wired, so the button that opens it can appear.
  toggle.hidden = false;
})();
