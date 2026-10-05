// ============================================================================
// The two filter boxes: one over the Index table, one over the navigation
// sidebar's rule list.
//
// Both narrow a list of lint names to what the reader types, hiding the
// entries that don't match and reordering the rest best-match-first. They
// run the identical logic over different lists, so `installFilter` is
// called twice and holds no knowledge of which list it drives; the scoring,
// the score threshold and the highlight rendering all come from match.js.
//
// Each box lives inside an empty `<div class="filter-container">` the Rust
// template emits. The container has no box of its own, so a page whose
// script never runs is pixel-identical to one without the feature — the
// input only exists once this file builds it. The toggle buttons follow the
// same "reveal only once functional" contract the nav hamburger, the
// settings gear and the Configuration bulk buttons do: the template emits
// them `hidden` and `installFilter` clears that as its last act, so a
// CSP-blocked, stripped or mid-parse-error script leaves no dead control
// behind (the `[hidden] { display: none !important }` reset in
// style/base.css is what keeps `hidden` authoritative). Reading match.js's
// threshold into `FILTER_MIN_SCORE` below is part of that contract: if
// match.js never ran, the read throws here, long before any reveal.
//
// Closing a box clears its query. A hidden input still filtering the list
// would otherwise leave entries missing with nothing on screen to explain
// why.
//
// Filtering runs synchronously on every keystroke, with no debounce: it is
// a string scan over a few dozen lint names, so the list keeps up with the
// keyboard as it is.
//
// ---- Keyboard entry -------------------------------------------------------
//
// While the Index table is on screen, typing a letter opens the Index box
// and seeds it with that letter, so finding a rule costs no clicks at all.
// The handler is deliberately narrow, because the page is a document first
// and a letter has to keep meaning what it means everywhere else:
//
//   * it does nothing unless the Index table is actually in view;
//   * it ignores anything with a modifier held, so browser and OS
//     shortcuts are untouched;
//   * it ignores keys that aren't a single letter, leaving Tab, Enter,
//     the arrows and every named key alone;
//   * it ignores a keystroke aimed at a form control or editable element,
//     which is also what keeps it off the search overlay's own input;
//   * it ignores a keystroke while the table sits inside an `inert`
//     subtree, which is how the nav drawer and the search overlay mark the
//     page behind them;
//   * it ignores an in-progress IME composition, where the keystroke
//     belongs to the composition rather than to the page.
//
// `preventDefault` is called only on the keystroke it goes on to handle,
// never on one it declined — the box sets the character itself, and without
// that the browser would insert it a second time once the input has focus.
// ============================================================================

(function () {
  // Also the load-bearing check that match.js ran: see the file header.
  var FILTER_MIN_SCORE = perfectionistMatch.FILTER_MIN_SCORE;

  /**
   * One filterable entry: the row or list item to show, hide and reorder,
   * the element whose contents spell the lint name (rebuilt to carry the
   * highlight), the name itself, and the entry's position in the rendered
   * order so a cleared query can put it back.
   * @typedef {object} FilterItem
   * @property {HTMLElement} element
   * @property {HTMLElement} nameHost
   * @property {string} name
   * @property {number} order
   */

  /**
   * One wired-up filter box, as `installFilter` hands it back.
   * @typedef {object} FilterBox
   * @property {(seed: string) => void} openWith
   */

  /**
   * Collect the filterable entries of a list.
   * @param {HTMLElement} list       the parent whose children are the entries
   * @param {string} itemSelector    which children count as entries
   * @param {string} nameSelector    the element inside an entry spelling the name
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
   *
   * Resolution is split from `wireFilter` so that function can take
   * non-null elements: TypeScript does not carry a guard's narrowing of a
   * `var` into a closure, so handlers reading a nullable `toggle` or
   * `list` would see it as nullable however it was guarded (the same
   * reason the other scripts cast their queries).
   * @param {string} kind
   * @param {string} listSelector  the parent whose children are the entries
   * @param {string} itemSelector  which children count as entries
   * @param {string} nameSelector  the element inside an entry spelling the name
   * @param {string} label         the input's accessible name
   * @returns {FilterBox | null}
   */
  function installFilter(kind, listSelector, itemSelector, nameSelector, label) {
    var toggle = document.querySelector("." + kind + "-filter-toggle");
    var container = document.querySelector("." + kind + "-filter-container");
    var list = document.querySelector(listSelector);
    if (!(toggle instanceof HTMLElement)) return null;
    if (!(container instanceof HTMLElement)) return null;
    if (!(list instanceof HTMLElement)) return null;
    return wireFilter(toggle, container, list, itemSelector, nameSelector, label);
  }

  /**
   * Build a filter input into `container`, let `toggle` show and hide it,
   * narrow the list's entries to whatever is typed, and finally reveal
   * `toggle`. Returns `null` for an empty list, leaving that toggle
   * hidden.
   * @param {HTMLElement} toggle
   * @param {HTMLElement} container
   * @param {HTMLElement} list
   * @param {string} itemSelector
   * @param {string} nameSelector
   * @param {string} label
   * @returns {FilterBox | null}
   */
  function wireFilter(toggle, container, list, itemSelector, nameSelector, label) {
    var items = collectItems(list, itemSelector, nameSelector);
    // Nothing to narrow means nothing to reveal: a toggle over an empty
    // list would open a box that can only ever hide nothing.
    if (items.length === 0) return null;

    var box = document.createElement("div");
    box.className = "filter-box";
    box.hidden = true;

    var input = document.createElement("input");
    input.className = "filter-input";
    // `type="search"` earns the platform's search affordances: a clear
    // button on desktop, a dedicated key on phone and tablet keyboards.
    input.type = "search";
    input.placeholder = "Filter by name\u2026";
    input.setAttribute("aria-label", label);
    // Phone and tablet keyboards otherwise capitalise the first letter and
    // autocorrect a half-typed lint name into a dictionary word, neither of
    // which can match a snake_case identifier.
    input.setAttribute("autocapitalize", "none");
    input.setAttribute("autocorrect", "off");
    input.setAttribute("autocomplete", "off");
    input.setAttribute("enterkeyhint", "search");
    input.spellcheck = false;
    box.appendChild(input);
    container.appendChild(box);

    /**
     * Put every entry back the way the page rendered it: visible, in
     * document order, with no highlight and no score.
     */
    function reset() {
      for (var i = 0; i < items.length; i++) {
        items[i].element.hidden = false;
        items[i].element.removeAttribute("data-score");
        perfectionistMatch.renderName(items[i].nameHost, items[i].name, []);
        // Re-appending in `items` order restores the rendered order, since
        // `items` was collected in it.
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
        var hit = perfectionistMatch.matchFuzzy(query, items[i].name);
        if (hit && hit.score >= FILTER_MIN_SCORE) {
          matched.push({ item: items[i], score: hit.score, ranges: hit.ranges });
          continue;
        }
        items[i].element.hidden = true;
        items[i].element.removeAttribute("data-score");
        perfectionistMatch.renderName(items[i].nameHost, items[i].name, []);
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
        // The score the entry was ranked by, for whoever is debugging a
        // ranking that reads wrong.
        entry.item.element.setAttribute("data-score", entry.score.toFixed(4));
        perfectionistMatch.renderName(entry.item.nameHost, entry.item.name, entry.ranges);
        list.appendChild(entry.item.element);
      }
    }

    /** @param {boolean} open */
    function setOpen(open) {
      toggle.setAttribute("aria-expanded", String(open));
      box.hidden = !open;
      if (open) {
        input.focus();
        return;
      }
      // A closed box must not keep filtering, so clear the query and put
      // the list back.
      input.value = "";
      reset();
    }

    toggle.addEventListener("click", function () {
      setOpen(toggle.getAttribute("aria-expanded") !== "true");
    });

    // `input` is the event that covers every way text arrives — physical
    // keyboard, on-screen keyboard, IME, paste, drag, the native clear
    // button. The others are belt and braces: `search` is what a
    // `type="search"` input fires on its clear button in older WebKit, and
    // Enter is the explicit re-run for a browser whose `input` event never
    // came.
    input.addEventListener("input", apply);
    input.addEventListener("search", apply);
    input.addEventListener("keydown", function (event) {
      if (event.key !== "Enter") return;
      // The input is in no form, so Enter submits nothing; suppressing the
      // default only keeps a stray form association from navigating.
      event.preventDefault();
      apply();
    });

    // Wired up, so the button that opens it can appear.
    toggle.hidden = false;

    return {
      /**
       * Open the box with `seed` as its contents, as the keyboard entry
       * path does. It overwrites whatever was typed before, matching the
       * reader's intent: the keystroke starts a fresh query.
       * @param {string} seed
       */
      openWith: function (seed) {
        setOpen(true);
        input.value = seed;
        apply();
      },
    };
  }

  var indexFilter = installFilter(
    "index",
    "table.index tbody",
    "tr",
    "td:first-child code",
    "Filter the index by lint name"
  );

  installFilter(
    "nav",
    "ul.nav-sidebar-list",
    "li",
    "a code",
    "Filter the navigation by lint name"
  );

  // ---- Keyboard entry into the Index box --------------------------------
  //
  // See the file header for why each guard below is there.
  var indexTable = document.querySelector("table.index");
  if (!indexFilter || !(indexTable instanceof HTMLElement)) return;
  // Bound to locals the guard above has already narrowed, for the reason
  // `installFilter` splits: the keydown handler below is a closure.
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

  document.addEventListener("keydown", function (event) {
    if (!indexInView) return;
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.isComposing) return;
    if (event.key.length !== 1) return;
    var letter = event.key.toLowerCase();
    if (letter < "a" || letter > "z") return;
    if (isEditable(event.target)) return;
    // `inert` is how the nav drawer and the search overlay mark the page
    // behind them; a letter typed over either belongs to them, not here.
    if (table.closest("[inert]")) return;
    event.preventDefault();
    filter.openWith(letter);
  });
})();
