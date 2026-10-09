// ============================================================================
// The filter boxes: one over the Index table, one over the navigation
// sidebar's rule list, narrowing a list of lint names to what the reader
// types.
// ============================================================================

;(function () {
  // Load-bearing: if either library never ran, this throws before any
  // control is revealed.
  var matchFuzzy = perfectionistMatch.matchFuzzy
  var renderName = perfectionistHighlight.renderName

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
   * @typedef {{ item: FilterItem, score: number, ranges: Span[] }} Matched
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
    var items = []
    var candidates = list.querySelectorAll(itemSelector)
    for (var i = 0; i < candidates.length; i++) {
      var element = candidates[i]
      if (!(element instanceof HTMLElement)) {
        continue
      }
      var nameHost = element.querySelector(nameSelector)
      if (!(nameHost instanceof HTMLElement)) {
        continue
      }
      var name = nameHost.textContent || ''
      if (!name) {
        continue
      }
      items.push({
        element: element,
        nameHost: nameHost,
        name: name,
        order: items.length,
      })
    }
    return items
  }

  /**
   * Does a keystroke aimed at this element belong to the element rather
   * than to the page?
   * @param {EventTarget | null} target
   * @returns {boolean}
   */
  function isEditable(target) {
    if (!(target instanceof HTMLElement)) {
      return false
    }
    if (target.isContentEditable) {
      return true
    }
    var tag = target.tagName
    return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT'
  }

  /**
   * Find one filter box's markup and wire it up.
   * @param {string} kind  `index` or `nav`
   * @param {string} listSelector  the parent whose children are the entries
   * @param {string} itemSelector  which children count as entries
   * @param {string} nameSelector  the element inside an entry spelling the name
   * @returns {FilterBox | null}
   */
  function installFilter(kind, listSelector, itemSelector, nameSelector) {
    var toggle = document.querySelector('.' + kind + '-filter-toggle')
    var container = document.querySelector('.' + kind + '-filter-container')
    var list = document.querySelector(listSelector)
    if (!(toggle instanceof HTMLElement)) {
      return null
    }
    if (!(container instanceof HTMLElement)) {
      return null
    }
    if (!(list instanceof HTMLElement)) {
      return null
    }
    return wireFilter(toggle, container, list, itemSelector, nameSelector)
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
    var items = collectItems(list, itemSelector, nameSelector)
    if (items.length === 0) {
      return null
    }

    // A browser without `<template>` parses it as an unknown element and
    // fails this check, leaving the funnel hidden.
    var blueprint = container.querySelector('template')
    if (!(blueprint instanceof HTMLTemplateElement)) {
      return null
    }
    container.appendChild(blueprint.content.cloneNode(true))
    var box = container.querySelector('.filter-box')
    var input = container.querySelector('.filter-input')
    if (!(box instanceof HTMLElement)) {
      return null
    }
    if (!(input instanceof HTMLInputElement)) {
      return null
    }
    return wireBox(toggle, list, items, box, input)
  }

  /**
   * @param {HTMLElement} toggle
   * @param {HTMLElement} list
   * @param {FilterItem[]} items
   * @param {HTMLElement} box
   * @param {HTMLInputElement} input
   * @returns {FilterBox}
   */
  function wireBox(toggle, list, items, box, input) {
    function reset() {
      for (var i = 0; i < items.length; i++) {
        var item = /** @type {FilterItem} */ (items[i])
        item.element.hidden = false
        item.element.removeAttribute('data-score')
        renderName(item.nameHost, item.name, [])
        // `items` was collected in rendered order, so re-appending in that
        // order restores it.
        list.appendChild(item.element)
      }
    }

    function apply() {
      var query = input.value.trim()
      if (query === '') {
        reset()
        return
      }
      /** @type {Matched[]} */
      var matched = []
      for (var i = 0; i < items.length; i++) {
        var item = /** @type {FilterItem} */ (items[i])
        var hit = matchFuzzy(query, item.name)
        if (hit) {
          matched.push({ item: item, score: hit.score, ranges: hit.ranges })
          continue
        }
        item.element.hidden = true
        item.element.removeAttribute('data-score')
        renderName(item.nameHost, item.name, [])
      }
      // The tie-break is explicit: sort stability cannot be assumed on the
      // engines this page targets.
      matched.sort(function (left, right) {
        if (right.score !== left.score) {
          return right.score - left.score
        }
        return left.item.order - right.item.order
      })
      for (var j = 0; j < matched.length; j++) {
        var entry = /** @type {Matched} */ (matched[j])
        entry.item.element.hidden = false
        entry.item.element.setAttribute('data-score', entry.score.toFixed(4))
        renderName(entry.item.nameHost, entry.item.name, entry.ranges)
        list.appendChild(entry.item.element)
      }
    }

    function openBox() {
      toggle.setAttribute('aria-expanded', 'true')
      box.hidden = false
      input.focus()
    }

    // A hidden input still narrowing a list would strand the reader with
    // entries missing and nothing on screen to say why.
    function closeBox() {
      input.value = ''
      reset()
      toggle.setAttribute('aria-expanded', 'false')
      box.hidden = true
    }

    /**
     * Close the box and put focus on the funnel: the input holding it is
     * about to be hidden, and focus would otherwise drop to <body>.
     */
    function dismiss() {
      closeBox()
      toggle.focus({ preventScroll: true })
    }

    toggle.addEventListener('click', function () {
      if (toggle.getAttribute('aria-expanded') === 'true') {
        dismiss()
      } else {
        openBox()
      }
    })

    // No debounce; the scan is over lint names.
    input.addEventListener('input', apply)
    // Belt and braces: what a `type="search"` input fires on its clear
    // button in older WebKit.
    input.addEventListener('search', apply)
    input.addEventListener('keydown', function (event) {
      // Neither key means anything to an IME mid-composition, where Enter
      // accepts the candidate and Escape abandons it.
      if (event.isComposing) {
        return
      }
      if (event.key === 'Enter') {
        // The input is in no form, so Enter submits nothing; suppressing
        // the default only keeps a stray form association from navigating.
        event.preventDefault()
        apply()
        return
      }
      if (event.key !== 'Escape') {
        return
      }
      // A `type="search"` input clears itself on Escape in WebKit and
      // Blink, which is half of what should happen here; suppressing the
      // default and doing the whole of it keeps every engine alike.
      event.preventDefault()
      // The innermost thing Escape can dismiss should be the only thing it
      // dismisses, so a handler further out does not shut something else on
      // the same keystroke.
      event.stopPropagation()
      dismiss()
    })

    // Following an entry leaves the reader the whole list to come back to.
    // Focus stays where the click sends it. A modifier-key or non-primary
    // click opens a new tab and leaves this page where it was, so its query
    // has to stay too.
    list.addEventListener('click', function (event) {
      var target = event.target
      if (!(target instanceof Element)) {
        return
      }
      var link = target.closest('a')
      if (!link) {
        return
      }
      if (event.button !== 0) {
        return
      }
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
        return
      }
      closeBox()
    })

    // Nothing appears that cannot yet be used.
    toggle.hidden = false

    return {
      /**
       * Open the box with `seed` as its contents, replacing whatever was
       * typed before: the keystroke starts a fresh query.
       * @param {string} seed
       */
      openWith: function (seed) {
        openBox()
        input.value = seed
        apply()
      },
    }
  }

  var indexFilter = installFilter('index', 'table.index tbody', 'tr', 'td:first-child code')

  installFilter('nav', 'ul.nav-sidebar-list', 'li', 'a code')

  // ---- Keyboard entry into the Index box --------------------------------
  var indexTable = document.querySelector('table.index')
  if (!indexFilter || !(indexTable instanceof HTMLElement)) {
    return
  }
  var table = indexTable
  var filter = indexFilter

  // Without IntersectionObserver the flag simply stays true and typing
  // works wherever the reader is — a wider trigger than intended, not a
  // broken one.
  var indexInView = true
  if ('IntersectionObserver' in window) {
    var observer = new IntersectionObserver(function (entries) {
      var latest = /** @type {IntersectionObserverEntry} */ (entries[entries.length - 1])
      indexInView = latest.isIntersecting
    })
    observer.observe(table)
  }

  // Without `preventDefault` the browser inserts the letter again once the
  // input has focus, doubling the one the box is seeded with.
  document.addEventListener('keydown', function (event) {
    if (!indexInView) {
      return
    }
    if (event.altKey || event.ctrlKey || event.metaKey) {
      return
    }
    if (event.isComposing) {
      return
    }
    // A named key spells itself out in `key`, so one character is how a
    // letter is told from `Tab` or an arrow.
    if (event.key.length !== 1) {
      return
    }
    // Seeded as typed; only the range test folds.
    var letter = event.key
    var folded = letter.toLowerCase()
    if (folded < 'a' || folded > 'z') {
      return
    }
    if (isEditable(event.target)) {
      return
    }
    // Typing belongs to whatever covers the page, where something does,
    // rather than to the table underneath it.
    if (table.closest('[inert]')) {
      return
    }
    event.preventDefault()
    filter.openWith(letter)
  })
})()
