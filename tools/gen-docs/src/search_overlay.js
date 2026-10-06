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
// No markup is written here. The overlay and one search result each live
// in an inert `<template>` rendered by the Rust template; this file clones
// them and wires up the behaviour. A `<template>`'s contents are parsed but
// kept out of the document — nothing renders, nothing is focusable,
// assistive tech never reaches them, and `document.querySelector` does not
// descend into them — so a page whose script never runs has no overlay in
// any sense that counts, while the markup stays where the rest of the
// page's markup is reviewed and tested.
//
// The Rust template emits the button `hidden` and this file clears that as
// its last act, the same "reveal only once functional" contract the rest of
// the page's controls follow (the `[hidden] { display: none !important }`
// reset in style/base.css keeps `hidden` authoritative). Reading rank.js's
// and highlight.js's entry points into locals below is part of it: if
// either never ran, that read throws here, long before the reveal.
//
// ---- Where the searchable text comes from ---------------------------------
//
// The catalogue is one document holding every rule's prose, so the search
// reads the page it is on instead of shipping a copy of that prose as a
// second payload. Nothing can drift, nothing is downloaded twice, and the
// scrape costs nothing until the reader first opens the overlay, at which
// point it runs once and is kept.
//
// The text that comes out of each `article.rule` is of these kinds, which
// rank.js weights in this order:
//
//   1. the lint name, from the heading;
//   2. the rule's statement — its one-line description, the paragraph
//      carrying the default-state badge;
//   3. its prose, one entry per paragraph and list item.
//
// A weaker match on a name therefore still outranks a perfect match in
// prose, which is what a reader scanning for a half-remembered lint wants.
// How much weaker, and everything else about the ordering, is rank.js's;
// this file only hands it the text.
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
  // Also the load-bearing check that the two libraries this file is
  // nothing without have run: see the file header.
  var rank = perfectionistRank.rank;
  var renderName = perfectionistHighlight.renderName;
  var renderText = perfectionistHighlight.renderText;

  var toggle = /** @type {HTMLElement} */ (document.querySelector(".search-toggle"));
  if (!toggle) return;

  // ---- Cloning the overlay into the page --------------------------------

  var overlayBlueprint = document.getElementById("search-overlay-template");
  var resultBlueprint = document.getElementById("search-result-template");
  // A browser without `<template>` parses both as unknown elements, fails
  // these checks and leaves the magnifier hidden, which is the right
  // outcome: there is nothing for it to open.
  if (!(overlayBlueprint instanceof HTMLTemplateElement)) return;
  if (!(resultBlueprint instanceof HTMLTemplateElement)) return;
  // The result blueprint's shape is checked once, here, rather than per
  // result: it is fixed markup, so if it is wrong it is wrong every time —
  // and failing now, before the reveal, leaves no dead button behind.
  if (!resultBlueprint.content.querySelector(".search-result")) return;
  if (!resultBlueprint.content.querySelector(".search-result-name")) return;
  if (!resultBlueprint.content.querySelector(".search-result-text")) return;
  // Bound to a local the guard above has already narrowed, because
  // `renderResult` is a closure and TypeScript does not carry a guard's
  // narrowing of a `var` into one.
  var resultTemplate = resultBlueprint;

  document.body.appendChild(overlayBlueprint.content.cloneNode(true));
  // Cast rather than narrowed, for the same reason: every handler below is
  // a closure. The guard still rejects a missing element at runtime.
  var overlay = /** @type {HTMLElement} */ (document.querySelector(".search-overlay"));
  if (!overlay) return;
  var dialog = /** @type {HTMLElement} */ (overlay.querySelector(".search-dialog"));
  var input = /** @type {HTMLInputElement} */ (overlay.querySelector(".search-input"));
  var close = /** @type {HTMLElement} */ (overlay.querySelector(".search-close"));
  var results = /** @type {HTMLElement} */ (overlay.querySelector(".search-results"));
  var emptyPrompt = /** @type {HTMLElement} */ (
    overlay.querySelector(".search-empty-prompt")
  );
  var emptyNoMatch = /** @type {HTMLElement} */ (
    overlay.querySelector(".search-empty-no-match")
  );
  if (!dialog || !input || !close || !results) return;
  if (!emptyPrompt || !emptyNoMatch) return;

  // ---- Scraping the page ------------------------------------------------

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
        var text = perfectionistRank.prose(blocks[j].textContent || "");
        if (text) paragraphs.push(text);
      }
      out.push({
        name: perfectionistRank.flatten(nameHost.textContent || ""),
        href: "#" + id,
        statement: statementHost
          ? perfectionistRank.flatten(textWithout(statementHost, badge))
          : "",
        paragraphs: paragraphs,
        order: out.length,
      });
    }
    return out;
  }

  // ---- Ranking ----------------------------------------------------------

  // ---- Rendering --------------------------------------------------------

  /**
   * Clone the result blueprint and fill it in. The casts are safe because
   * the blueprint's shape was checked once at setup.
   * @param {Result} result
   */
  function renderResult(result) {
    var item = /** @type {DocumentFragment} */ (resultTemplate.content.cloneNode(true));
    var link = /** @type {HTMLAnchorElement} */ (item.querySelector(".search-result"));
    var name = /** @type {HTMLElement} */ (item.querySelector(".search-result-name"));
    var text = /** @type {HTMLElement} */ (item.querySelector(".search-result-text"));
    link.href = result.entry.href;
    // The score the result was ranked by, for whoever is debugging a
    // ranking that reads wrong.
    link.setAttribute("data-score", result.score.toFixed(4));
    renderName(name, result.entry.name, result.nameRanges);
    renderText(text, result.text, result.textRanges);
    results.appendChild(item);
  }

  /**
   * Reveal one of the results list and the two messages, and hide the
   * other two. Each claims the dialog's whole remaining height, so
   * leaving two showing would halve both.
   * @param {HTMLElement} shown
   */
  function showOnly(shown) {
    results.hidden = shown !== results;
    emptyPrompt.hidden = shown !== emptyPrompt;
    emptyNoMatch.hidden = shown !== emptyNoMatch;
  }

  function apply() {
    while (results.firstChild) results.removeChild(results.firstChild);
    var query = input.value.trim();
    // Nothing typed yet, so say what typing will do rather than leave the
    // dialog a blank panel.
    if (query === "") {
      showOnly(emptyPrompt);
      return;
    }
    if (!entries) entries = scrape();
    var ranked = rank(entries, query);
    // A query that matches nothing is worth saying so: an empty list
    // reads the same as one that has not been searched yet.
    if (ranked.length === 0) {
      showOnly(emptyNoMatch);
      return;
    }
    showOnly(results);
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

  // ---- Up and Down move focus, as Tab does ------------------------------
  //
  // The browser's own sequential focus navigation cannot be asked for: a
  // `KeyboardEvent` built in script carries `isTrusted: false`, and an
  // untrusted event never performs a default action, so dispatching a
  // synthetic Tab moves nothing. What is reused is the focus itself —
  // `element.focus()` is the same focus Tab arrives at, `:focus-visible`
  // and all — leaving only the order to be written out, which is the
  // document order Tab already walks.
  //
  // The keys are taken while the caret is still in the input, so the
  // default has to go with them: Up and Down would otherwise jump the
  // caret to the ends of the value. Home and End still do that.

  /**
   * Everything in the overlay that can take focus, in the order Tab
   * reaches it. The results list is `hidden` until there is something in
   * it and the two empty states are hidden in turn, so filtering on
   * whether an element is rendered is what keeps the order honest.
   * @returns {HTMLElement[]}
   */
  function focusables() {
    var found = overlay.querySelectorAll(
      "a[href], button:not([disabled]), input:not([disabled])"
    );
    /** @type {HTMLElement[]} */
    var out = [];
    for (var i = 0; i < found.length; i++) {
      var element = /** @type {HTMLElement} */ (found[i]);
      if (element.getClientRects().length > 0) out.push(element);
    }
    return out;
  }

  overlay.addEventListener("keydown", function (event) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    // A modifier turns these into something the browser or the OS owns,
    // and an arrow mid-composition belongs to the IME.
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    if (event.isComposing) return;
    var items = focusables();
    if (items.length === 0) return;
    var step = event.key === "ArrowDown" ? 1 : -1;
    var at = items.indexOf(/** @type {HTMLElement} */ (document.activeElement));
    // Wrapping is what Tab does here. Every other child of <body> is
    // inert while the overlay is open, so there is nowhere else to go.
    var next =
      at < 0
        ? step > 0
          ? 0
          : items.length - 1
        : (at + step + items.length) % items.length;
    event.preventDefault();
    items[next].focus();
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
