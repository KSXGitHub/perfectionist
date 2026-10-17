// ============================================================================
// The Search overlay: a modal box over the page that ranks whole rules
// against a query. Each result links to the rule's own fragment, so
// following one lands the reader on the rule article itself.
//
// No markup is written here: the overlay and one search result each live in
// a `<template>` this file clones. Nothing inside one renders, takes focus
// or answers `querySelector`, so a page whose script never runs has no
// overlay in any sense that counts.
//
// The catalogue is one document holding every rule's prose, so the search
// reads the page it is on instead of shipping a copy of that prose as a
// second payload: nothing can drift, nothing is downloaded twice, and the
// scrape costs nothing until the reader first opens the overlay, at which
// point it runs once and is kept.
// ============================================================================

;(function () {
  // Load-bearing: if either library never ran, this throws before anything
  // is revealed.
  var rank = perfectionistRank.rank
  var renderName = perfectionistHighlight.renderName
  var renderText = perfectionistHighlight.renderText

  // ---- Cloning the overlay into the page --------------------------------

  /**
   * @typedef {object} Parts
   * @property {HTMLElement} toggle
   * @property {HTMLTemplateElement} resultBlueprint
   * @property {HTMLElement} overlay
   * @property {HTMLElement} dialog
   * @property {HTMLInputElement} input
   * @property {Element} close
   * @property {HTMLElement} results
   * @property {HTMLElement} emptyPrompt
   * @property {HTMLElement} emptyNoMatch
   */

  /**
   * Clone the overlay into the page and hand back its parts, or `null` if
   * anything is missing or not the element it should be. Each part is
   * proven here so the behaviour below needs no assertion about any of
   * them.
   * @returns {Parts | null}
   */
  function buildOverlay() {
    var toggle = document.querySelector('.search-toggle')
    if (!(toggle instanceof HTMLElement)) {
      return null
    }
    // A browser without `<template>` parses both as unknown elements, fails
    // these checks and leaves the magnifier hidden, which is the right
    // outcome: there is nothing for it to open.
    var overlayBlueprint = document.getElementById('search-overlay-template')
    var resultBlueprint = document.getElementById('search-result-template')
    if (!(overlayBlueprint instanceof HTMLTemplateElement)) {
      return null
    }
    if (!(resultBlueprint instanceof HTMLTemplateElement)) {
      return null
    }
    // The result blueprint's shape is checked once, here, rather than per
    // result: it is fixed markup, so if it is wrong it is wrong every time —
    // and failing now, before the reveal, leaves no dead button behind.
    if (!resultBlueprint.content.querySelector('.search-result')) {
      return null
    }
    if (!resultBlueprint.content.querySelector('.search-result-name')) {
      return null
    }
    if (!resultBlueprint.content.querySelector('.search-result-text')) {
      return null
    }

    document.body.appendChild(overlayBlueprint.content.cloneNode(true))
    var overlay = document.querySelector('.search-overlay')
    if (!(overlay instanceof HTMLElement)) {
      return null
    }
    var dialog = overlay.querySelector('.search-dialog')
    var input = overlay.querySelector('.search-input')
    var close = overlay.querySelector('.search-close')
    var results = overlay.querySelector('.search-results')
    var emptyPrompt = overlay.querySelector('.search-empty-prompt')
    var emptyNoMatch = overlay.querySelector('.search-empty-no-match')
    if (!(dialog instanceof HTMLElement)) {
      return null
    }
    if (!(input instanceof HTMLInputElement)) {
      return null
    }
    if (!close) {
      return null
    }
    if (!(results instanceof HTMLElement)) {
      return null
    }
    if (!(emptyPrompt instanceof HTMLElement)) {
      return null
    }
    if (!(emptyNoMatch instanceof HTMLElement)) {
      return null
    }
    return {
      toggle: toggle,
      resultBlueprint: resultBlueprint,
      overlay: overlay,
      dialog: dialog,
      input: input,
      close: close,
      results: results,
      emptyPrompt: emptyPrompt,
      emptyNoMatch: emptyNoMatch,
    }
  }

  var parts = buildOverlay()
  if (!parts) {
    return
  }
  var toggle = parts.toggle
  var resultBlueprint = parts.resultBlueprint
  var overlay = parts.overlay
  var dialog = parts.dialog
  var input = parts.input
  var close = parts.close
  var results = parts.results
  var emptyPrompt = parts.emptyPrompt
  var emptyNoMatch = parts.emptyNoMatch

  // ---- Scraping the page ------------------------------------------------

  /** @type {readonly Entry[] | null} */
  var entries = null

  /**
   * The text of `parent`, with one child node left out.
   * @param {Element} parent
   * @param {Node | null} omit
   * @returns {string}
   */
  function textWithout(parent, omit) {
    var out = ''
    for (var i = 0; i < parent.childNodes.length; i++) {
      var node = parent.childNodes[i]
      if (node === omit) {
        continue
      }
      out += node.textContent || ''
    }
    return out
  }

  /**
   * Does every rule repeat this block verbatim? Matching text that every
   * rule carries tells the reader nothing about which rule they want, so
   * it is left out of the scrape.
   * @param {HTMLElement} block
   * @returns {boolean}
   */
  function isRepeated(block) {
    if (block.classList.contains('source')) {
      return true
    }
    if (block.classList.contains('config-none')) {
      return true
    }
    var parent = block.parentElement
    return !!parent && parent.matches('details.config-details')
  }

  /**
   * Read every rule off the page. Called once, on the first open.
   * @returns {readonly Entry[]}
   */
  function scrape() {
    /** @type {Entry[]} */
    var out = []
    var articles = document.querySelectorAll('article.rule')
    for (var i = 0; i < articles.length; i++) {
      var article = articles[i]
      var nameHost = article.querySelector('h2 .lint-name')
      var id = article.getAttribute('id')
      if (!nameHost || !id) {
        continue
      }
      // The default-state badge marks the statement paragraph; the
      // statement is that paragraph minus the badge's own word.
      var badge = article.querySelector('p .state')
      var statementHost = badge ? badge.parentElement : null
      /** @type {string[]} */
      var paragraphs = []
      var blocks = article.querySelectorAll('p, li')
      for (var j = 0; j < blocks.length; j++) {
        var block = blocks[j]
        if (!(block instanceof HTMLElement)) {
          continue
        }
        if (block === statementHost || isRepeated(block)) {
          continue
        }
        var text = perfectionistRank.prose(block.textContent || '')
        if (text) {
          paragraphs.push(text)
        }
      }
      out.push({
        name: perfectionistRank.flatten(nameHost.textContent || ''),
        href: '#' + id,
        statement: statementHost ? perfectionistRank.flatten(textWithout(statementHost, badge)) : '',
        paragraphs: paragraphs,
        order: out.length,
      })
    }
    return out
  }

  // ---- The selected result ----------------------------------------------
  //
  // The selection is not focus: the caret stays in the input throughout,
  // so a reader can go on typing with a result selected. The highlight is
  // `aria-selected`, which search.css draws, and `aria-activedescendant`
  // on the input names the selected element for assistive tech.

  /**
   * The results now listed, rebuilt by `apply` on every keystroke.
   * @type {HTMLAnchorElement[]}
   */
  var rendered = []

  /**
   * The selected result, or `null` for none.
   * @type {HTMLAnchorElement | null}
   */
  var selected = null

  /**
   * Make `next` the selected result, or `null` to select nothing.
   * @param {HTMLAnchorElement | null} next
   */
  function select(next) {
    if (next === selected) {
      return
    }
    if (selected) {
      selected.setAttribute('aria-selected', 'false')
    }
    selected = next
    if (!next) {
      input.removeAttribute('aria-activedescendant')
      return
    }
    next.setAttribute('aria-selected', 'true')
    input.setAttribute('aria-activedescendant', next.id)
  }

  // ---- Rendering --------------------------------------------------------

  /**
   * Clone the result blueprint and fill it in.
   * @param {Result} result
   */
  function renderResult(result) {
    var item = /** @type {DocumentFragment} */ (resultBlueprint.content.cloneNode(true))
    var link = item.querySelector('.search-result')
    var name = item.querySelector('.search-result-name')
    var text = item.querySelector('.search-result-text')
    if (!(link instanceof HTMLAnchorElement)) {
      return
    }
    if (!(name instanceof HTMLElement) || !(text instanceof HTMLElement)) {
      return
    }
    link.href = result.entry.href
    // Unique within the page, because the input's
    // `aria-activedescendant` names the selected result by it.
    link.id = 'search-result-' + rendered.length
    // The score the result was ranked by, for whoever is debugging a
    // ranking that reads wrong.
    link.setAttribute('data-score', result.score.toFixed(4))
    renderName(name, result.entry.name, result.nameRanges)
    renderText(text, result.text, result.textRanges)
    rendered.push(link)
    results.appendChild(item)
  }

  /**
   * Reveal one of the results list and the two messages, and hide the
   * other two.
   * @param {HTMLElement} shown
   */
  function showOnly(shown) {
    results.hidden = shown !== results
    emptyPrompt.hidden = shown !== emptyPrompt
    emptyNoMatch.hidden = shown !== emptyNoMatch
    // The combobox's popup is the results list, so it is expanded
    // exactly when that list is the panel on show.
    input.setAttribute('aria-expanded', shown === results ? 'true' : 'false')
  }

  function apply() {
    // Given up before the list holding it is emptied, so `move` never
    // looks for a result that has left.
    select(null)
    rendered = []
    while (results.firstChild) {
      results.removeChild(results.firstChild)
    }
    var query = input.value.trim()
    // Nothing typed yet, so say what typing will do rather than leave the
    // dialog a blank panel.
    if (query === '') {
      showOnly(emptyPrompt)
      return
    }
    if (!entries) {
      entries = scrape()
    }
    var ranked = rank(entries, query)
    // A query that matches nothing is worth saying so: an empty list
    // reads the same as one that has not been searched yet.
    if (ranked.length === 0) {
      showOnly(emptyNoMatch)
      return
    }
    showOnly(results)
    for (var i = 0; i < ranked.length; i++) {
      renderResult(ranked[i])
    }
    // Pre-selected, so Enter follows the first result without an
    // arrow key.
    if (rendered.length > 0) {
      select(rendered[0])
    }
  }

  // ---- Background inertness --------------------------------------------
  //
  // The overlay is modal, so Tab must not wander into the page behind it
  // and assistive tech must not read it out. `inert` removes a subtree from
  // both in one step; it is applied to every direct child of <body> except
  // the overlay while it is open. A
  // child already inert is not recorded, so clearing undoes only what this
  // set. Browsers too old for `inert` ignore it and fall back to what the
  // page gives for free: the overlay is still dismissible by its close
  // button and by Escape.
  /** @type {HTMLElement[]} */
  var inerted = []

  function setBackgroundInert() {
    inerted = []
    for (var i = 0; i < document.body.children.length; i++) {
      var child = document.body.children[i]
      if (!(child instanceof HTMLElement)) {
        continue
      }
      if (child === overlay) {
        continue
      }
      if (child.inert) {
        continue
      }
      child.inert = true
      inerted.push(child)
    }
  }

  function clearBackgroundInert() {
    for (var i = 0; i < inerted.length; i++) {
      inerted[i].inert = false
    }
    inerted = []
  }

  // ---- Open / close -----------------------------------------------------

  function openOverlay() {
    overlay.hidden = false
    toggle.setAttribute('aria-expanded', 'true')
    setBackgroundInert()
    input.focus()
  }

  function closeOverlay() {
    overlay.hidden = true
    toggle.setAttribute('aria-expanded', 'false')
    clearBackgroundInert()
    // The button is what the reader came from and what reopens the
    // overlay, so focus goes back to it rather than dropping to <body>.
    toggle.focus({ preventScroll: true })
  }

  function isOpen() {
    return !overlay.hidden
  }

  toggle.addEventListener('click', function () {
    if (isOpen()) {
      closeOverlay()
    } else {
      openOverlay()
    }
  })

  close.addEventListener('click', closeOverlay)

  // A click on the backdrop — anywhere in the overlay outside the dialog
  // — dismisses it, the conventional gesture for a modal.
  overlay.addEventListener('click', function (event) {
    var target = event.target
    if (target instanceof Node && dialog.contains(target)) {
      return
    }
    closeOverlay()
  })

  // Escape closes the overlay. The default is suppressed only while the
  // overlay is open, so Escape keeps its ordinary meaning everywhere else
  // on the page.
  document.addEventListener('keydown', function (event) {
    if (event.key !== 'Escape') {
      return
    }
    // Mid-composition the key belongs to the IME, which abandons the
    // candidate on it. Closing the overlay here would dismiss the dialog
    // and commit the half-composed text into the input it just hid.
    if (event.isComposing) {
      return
    }
    if (!isOpen()) {
      return
    }
    event.preventDefault()
    closeOverlay()
  })

  // ---- `/` toggles the overlay ------------------------------------------
  //
  // The magnifier's keyboard equivalent: one key that opens the overlay
  // from anywhere on the page and closes it again. `/` is what a reader
  // arriving from the rest of the Rust documentation already has in their
  // fingers — rustdoc puts the caret in its own search box on it.
  //
  // Where a `/` means a slash it stays one, so the key toggles everywhere
  // except inside text entry, where a query may want the character.

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

  document.addEventListener('keydown', function (event) {
    if (event.key !== '/') {
      return
    }
    // A modifier turns the keystroke into something the browser or the OS
    // owns. Shift is not among them: on a layout where `/` is a shifted
    // key it is how the character is typed at all, and `key` is the
    // character either way.
    if (event.ctrlKey || event.altKey || event.metaKey) {
      return
    }
    if (event.isComposing) {
      return
    }
    if (isEditable(event.target)) {
      return
    }
    // `preventDefault` goes only on the keystroke each branch handles,
    // never on one it declined. On the way in it is load-bearing twice
    // over: it keeps Firefox's Quick Find shut, and it keeps the browser
    // from inserting the slash into the input this is about to focus.
    if (isOpen()) {
      event.preventDefault()
      closeOverlay()
      return
    }
    // A `/` typed over something covering the page belongs to that, not
    // here. Asked only on the way in, because opening the overlay inerts
    // this button too.
    if (toggle.closest('[inert]')) {
      return
    }
    event.preventDefault()
    openOverlay()
  })

  input.addEventListener('input', apply)
  input.addEventListener('search', apply)

  // ---- Enter follows the selected result --------------------------------
  //
  // Taken from the input alone rather than from the overlay, so that Enter
  // goes on pressing the Close button once Tab has moved there.
  input.addEventListener('keydown', function (event) {
    if (event.key !== 'Enter') {
      return
    }
    // Mid-composition Enter accepts the IME's candidate, and the `input`
    // event that follows re-runs the query anyway.
    if (event.isComposing) {
      return
    }
    // The input is in no form, so Enter submits nothing; suppressing the
    // default only keeps a stray form association from navigating.
    event.preventDefault()
    if (!selected) {
      return
    }
    // A click, so that following a result by Enter and by pointer is one
    // path: the handler the results list carries closes the overlay and
    // places focus, and the browser navigates.
    selected.click()
  })

  // ---- Up and Down move the selection -----------------------------------
  //
  // The keys are pressed with the caret still in the input, so the
  // default has to go with them: Up and Down would otherwise jump the
  // caret to the ends of the value. Home and End still do that.
  //
  // Bound to the overlay rather than to the input, because the selection
  // is the dialog's: moving it means the same wherever the caret is.

  /**
   * Move the selection by `step`. Called only with results listed, so
   * there is always one to land on.
   * @param {number} step
   */
  function move(step) {
    var at
    if (selected) {
      // Wrapping: these keys reach nothing but the list, so there is
      // nowhere an end could hand the reader on to.
      at = (rendered.indexOf(selected) + step + rendered.length) % rendered.length
    } else {
      // A list with nothing selected is entered at the end the key
      // points away from.
      at = step > 0 ? 0 : rendered.length - 1
    }
    var landed = rendered[at]
    select(landed)
    // `nearest`, so a selection the reader can already see stays put
    // rather than the list jumping under it.
    landed.scrollIntoView({ block: 'nearest' })
  }

  overlay.addEventListener('keydown', function (event) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') {
      return
    }
    // A modifier turns these into something the browser or the OS owns,
    // and an arrow mid-composition belongs to the IME.
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return
    }
    if (event.isComposing) {
      return
    }
    // Nothing to move, so the keys stay the browser's and go on
    // jumping the caret.
    if (rendered.length === 0) {
      return
    }
    event.preventDefault()
    move(event.key === 'ArrowDown' ? 1 : -1)
  })

  // ---- The pointer moves the selection ----------------------------------
  //
  // The pointer selects rather than hovers. A CSS `:hover` would draw a
  // second highlight that Enter does not follow, lit alongside the first
  // whenever the pointer rested on one result while the keys had moved
  // to another.

  results.addEventListener('mousemove', function (event) {
    var over = event.target
    if (!(over instanceof Element)) {
      return
    }
    var link = over.closest('.search-result')
    // A movement over the list's own padding is inside no result's box,
    // and leaves the selection where it is.
    if (!(link instanceof HTMLAnchorElement)) {
      return
    }
    select(link)
  })

  // A reader who has moved the pointer off the list is pointing at no
  // result, so Enter follows none.
  results.addEventListener('mouseleave', function () {
    select(null)
  })

  // Following a result closes the overlay so the rule it lands on is
  // visible, and moves focus there so a keyboard reader keeps their place
  // — the link they just activated is inside a now-hidden overlay, and
  // focus would otherwise drop to <body>. Modifier-key clicks and
  // non-primary buttons are "open in a new tab" gestures and must leave
  // this page as it stands. Rule articles aren't focusable by default, so
  // `tabindex="-1"` goes on first.
  results.addEventListener('click', function (event) {
    var clicked = event.target
    if (!(clicked instanceof Element)) {
      return
    }
    var link = clicked.closest('a')
    if (!link) {
      return
    }
    if (event.button !== 0) {
      return
    }
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return
    }
    closeOverlay()
    var hash = link.hash
    if (!hash) {
      return
    }
    // Rule fragments are `#/rule/<name>`, whose `/` characters make them
    // invalid CSS id selectors — `querySelector("#/rule/...")` would
    // throw. `getElementById` matches the literal `id` and accepts them.
    var target = document.getElementById(decodeURIComponent(hash.slice('#'.length)))
    if (!target) {
      return
    }
    target.setAttribute('tabindex', '-1')
    target.focus({ preventScroll: true })
  })

  // On mobile browsers that anchor `position: fixed` to the layout viewport
  // rather than the visual one, translate the button by the visual
  // viewport's offset so it stays glued to the top of the visible area as
  // the URL bar collapses. The overlay
  // itself is left alone — it is opened from a tap, and the on-screen
  // keyboard that follows shrinks the visual viewport, so pinning it to
  // that would shrink the dialog out from under the reader's fingers.
  if (window.visualViewport) {
    var vv = window.visualViewport
    var syncToViewport = function () {
      toggle.style.transform = 'translate(' + vv.offsetLeft + 'px, ' + vv.offsetTop + 'px)'
    }
    vv.addEventListener('scroll', syncToViewport)
    vv.addEventListener('resize', syncToViewport)
    syncToViewport()
  }

  // Everything is built and wired, so the button that opens it can appear.
  toggle.hidden = false
})()
