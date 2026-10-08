// ============================================================================
// The filter boxes: one over the Index table, one over the navigation
// sidebar's rule list, narrowing a list of lint names to what the reader
// types.
// ============================================================================

(function () {
  // Load-bearing: if either library never ran, this throws before any
  // control is revealed.
  var matchFuzzy = perfectionistMatch.matchFuzzy;
  var renderName = perfectionistHighlight.renderName;

  /**
   * One filterable entry. `order` is its place in the rendered list, which
   * is what a cleared query puts it back into.
   * @typedef {object} FilterItem
   * @property {HTMLElement} element
   * @property {HTMLElement} nameHost
   * @property {string} name
   * @property {number} order
   */

  /**
   * @typedef {object} FilterBox
   * @property {(seed: string) => void} openWith
   */

  /**
   * @param {HTMLElement} list
   * @param {string} itemSelector
   * @param {string} nameSelector
   * @returns {FilterItem[]}
   */
  function collectItems(list, itemSelector, nameSelector) {
    /** @type {FilterItem[]} */
    var items = [];
    var candidates = /** @type {NodeListOf<HTMLElement>} */ (
      list.querySelectorAll(itemSelector)
    );
    for (var i = 0; i < candidates.length; i++) {
      var nameHost = /** @type {HTMLElement | null} */ (
        candidates[i].querySelector(nameSelector)
      );
      if (!nameHost) continue;
      var name = nameHost.textContent || "";
      if (!name) continue;
      items.push({
        element: candidates[i],
        nameHost: nameHost,
        name: name,
        order: items.length,
      });
    }
    return items;
  }

  /**
   * Does a keystroke aimed at this element belong to the element rather
   * than to the page?
   * @param {EventTarget | null} target
   * @returns {boolean}
   */
  function isEditable(target) {
    if (!(target instanceof HTMLElement)) return false;
    if (target.isContentEditable) return true;
    var tag = target.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
  }

  /**
   * Find one filter box's markup and wire it up. `kind` is the prefix the
   * Rust template builds the toggle's and the container's class names
   * from — `index` or `nav` — so naming it here is naming both.
   * @param {string} kind
   * @param {string} listSelector  the parent whose children are the entries
   * @param {string} itemSelector  which children count as entries
   * @param {string} nameSelector  the element inside an entry spelling the name
   * @returns {FilterBox | null}
   */
  function installFilter(kind, listSelector, itemSelector, nameSelector) {
    var toggle = document.querySelector("." + kind + "-filter-toggle");
    var container = document.querySelector("." + kind + "-filter-container");
    var list = document.querySelector(listSelector);
    if (!(toggle instanceof HTMLElement)) return null;
    if (!(container instanceof HTMLElement)) return null;
    if (!(list instanceof HTMLElement)) return null;
    return wireFilter(toggle, container, list, itemSelector, nameSelector);
  }

  /**
   * @param {HTMLElement} toggle
   * @param {HTMLElement} container
   * @param {HTMLElement} list
   * @param {string} itemSelector
   * @param {string} nameSelector
   * @returns {FilterBox | null}
   */
  function wireFilter(toggle, container, list, itemSelector, nameSelector) {
    var items = collectItems(list, itemSelector, nameSelector);
    if (items.length === 0) return null;

    // A browser without `<template>` parses it as an unknown element and
    // fails this check, leaving the funnel hidden.
    var blueprint = container.querySelector("template");
    if (!(blueprint instanceof HTMLTemplateElement)) return null;
    container.appendChild(blueprint.content.cloneNode(true));
    // Cast rather than narrowed, for the reason `installFilter` splits;
    // the guard below still rejects a missing element at runtime.
    var box = /** @type {HTMLElement} */ (container.querySelector(".filter-box"));
    var input = /** @type {HTMLInputElement} */ (
      container.querySelector(".filter-input")
    );
    if (!box || !input) return null;

    function reset() {
      for (var i = 0; i < items.length; i++) {
        items[i].element.hidden = false;
        items[i].element.removeAttribute("data-score");
        renderName(items[i].nameHost, items[i].name, []);
        // `items` was collected in rendered order, so re-appending in that
        // order restores it.
        list.appendChild(items[i].element);
      }
    }

    function apply() {
      var query = input.value.trim();
      if (query === "") {
        reset();
        return;
      }
      /** @type {{ item: FilterItem, score: number, ranges: number[][] }[]} */
      var matched = [];
      for (var i = 0; i < items.length; i++) {
        // A match or nothing: whether a name is worth showing is
        // match.js's, and it decides that without reading the score. The
        // score only orders what is shown.
        var hit = matchFuzzy(query, items[i].name);
        if (hit) {
          matched.push({ item: items[i], score: hit.score, ranges: hit.ranges });
          continue;
        }
        items[i].element.hidden = true;
        items[i].element.removeAttribute("data-score");
        renderName(items[i].nameHost, items[i].name, []);
      }
      // Best match first, ties broken by rendered order. The tie-break is
      // explicit rather than left to the sort's stability, which engines
      // older than ES2019 don't guarantee.
      matched.sort(function (left, right) {
        if (right.score !== left.score) return right.score - left.score;
        return left.item.order - right.item.order;
      });
      for (var j = 0; j < matched.length; j++) {
        var entry = matched[j];
        entry.item.element.hidden = false;
        entry.item.element.setAttribute("data-score", entry.score.toFixed(4));
        renderName(entry.item.nameHost, entry.item.name, entry.ranges);
        list.appendChild(entry.item.element);
      }
    }

    function openBox() {
      toggle.setAttribute("aria-expanded", "true");
      box.hidden = false;
      input.focus();
    }

    /**
     * Escape, the funnel and following an entry all come here: a hidden
     * input still narrowing a list would strand the reader with entries
     * missing and nothing on screen to say why.
     */
    function closeBox() {
      input.value = "";
      reset();
      toggle.setAttribute("aria-expanded", "false");
      box.hidden = true;
    }

    /**
     * Close the box and put focus on the funnel. Escape dismisses from
     * inside the input, which is about to be hidden, and focus would
     * otherwise drop to <body>. Dismissing by the funnel click already has
     * focus there, so the move is a no-op on that path.
     */
    function dismiss() {
      closeBox();
      toggle.focus({ preventScroll: true });
    }

    toggle.addEventListener("click", function () {
      if (toggle.getAttribute("aria-expanded") === "true") {
        dismiss();
      } else {
        openBox();
      }
    });

    // `input` is the event that covers every way text arrives — physical
    // keyboard, on-screen keyboard, IME, paste, drag, the native clear
    // button. `search` is belt and braces: it is what a `type="search"`
    // input fires on its clear button in older WebKit.
    // No debounce; the scan is over lint names.
    input.addEventListener("input", apply);
    input.addEventListener("search", apply);
    input.addEventListener("keydown", function (event) {
      // Neither key means anything to an IME mid-composition, where Enter
      // accepts the candidate and Escape abandons it.
      if (event.isComposing) return;
      if (event.key === "Enter") {
        // The input is in no form, so Enter submits nothing; suppressing
        // the default only keeps a stray form association from navigating.
        event.preventDefault();
        apply();
        return;
      }
      if (event.key !== "Escape") return;
      // A `type="search"` input clears itself on Escape in WebKit and
      // Blink, which is half of what should happen here; suppressing the
      // default and doing the whole of it keeps every engine alike.
      event.preventDefault();
      // The innermost thing Escape can dismiss should be the only thing it
      // dismisses. theme_toggle.js listens for Escape on the document to
      // close the Settings panel, so without this one keystroke would shut
      // both the filter box and a panel the reader had left open.
      event.stopPropagation();
      dismiss();
    });

    // Following an entry leaves the reader the whole list to come back
    // to. Focus stays where the click sends it, unlike the paths above.
    // A modifier-key or non-primary click opens a new tab and leaves this
    // page where it was, so its query has to stay too.
    list.addEventListener("click", function (event) {
      var link = /** @type {Element} */ (event.target).closest("a");
      if (!link) return;
      if (event.button !== 0) return;
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      closeBox();
    });

    // Nothing appears that cannot yet be used.
    toggle.hidden = false;

    return {
      /**
       * Open the box with `seed` as its contents, replacing whatever was
       * typed before: the keystroke starts a fresh query.
       * @param {string} seed
       */
      openWith: function (seed) {
        openBox();
        input.value = seed;
        apply();
      },
    };
  }

  var indexFilter = installFilter(
    "index",
    "table.index tbody",
    "tr",
    "td:first-child code"
  );

  installFilter("nav", "ul.nav-sidebar-list", "li", "a code");

  // ---- Keyboard entry into the Index box --------------------------------
  var indexTable = document.querySelector("table.index");
  if (!indexFilter || !(indexTable instanceof HTMLElement)) return;
  // Locals, for the reason `installFilter` splits.
  var table = indexTable;
  var filter = indexFilter;

  // The index sits at the top of the page, so it starts in view. Without
  // IntersectionObserver the flag simply stays true and typing works
  // wherever the reader is — a wider trigger than intended, not a broken
  // one.
  var indexInView = true;
  if ("IntersectionObserver" in window) {
    var observer = new IntersectionObserver(function (entries) {
      indexInView = entries[entries.length - 1].isIntersecting;
    });
    observer.observe(table);
  }

  // A letter opens the box seeded with it. `preventDefault` goes only on
  // a keystroke this took, since the box sets the character itself and the
  // browser would otherwise insert it again once the input has focus.
  document.addEventListener("keydown", function (event) {
    if (!indexInView) return;
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.isComposing) return;
    // A named key spells itself out in `key`, so one character is how a
    // letter is told from `Tab` or an arrow.
    if (event.key.length !== 1) return;
    // Seeded as typed; only the range test folds.
    var letter = event.key;
    var folded = letter.toLowerCase();
    if (folded < "a" || folded > "z") return;
    if (isEditable(event.target)) return;
    // `inert` is how the nav drawer and the search overlay mark the page
    // behind them.
    if (table.closest("[inert]")) return;
    event.preventDefault();
    filter.openWith(letter);
  });
})();
