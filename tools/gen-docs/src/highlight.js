// ============================================================================
// Painting a match onto the page.
//
// Given an element, the text it should hold and the ranges of that text a
// query matched, this rebuilds the element's contents with each matched
// range wrapped in a `<mark>`.
// ============================================================================

var perfectionistHighlight = (function () {
  /**
   * Append `text.slice(from, to)` to `parent` as text nodes. With
   * `breakAfterUnderscore`, a `<wbr>` follows each `_` that isn't the last
   * character of `text`.
   * @param {Node} parent
   * @param {string} text
   * @param {number} from
   * @param {number} to
   * @param {boolean} breakAfterUnderscore
   */
  function appendRun(parent, text, from, to, breakAfterUnderscore) {
    if (from >= to) {
      return
    }
    if (!breakAfterUnderscore) {
      parent.appendChild(document.createTextNode(text.slice(from, to)))
      return
    }
    var start = from
    for (var i = from; i < to; i++) {
      if (text.charAt(i) !== '_' || i === text.length - 1) {
        continue
      }
      parent.appendChild(document.createTextNode(text.slice(start, i + 1)))
      parent.appendChild(document.createElement('wbr'))
      start = i + 1
    }
    if (start < to) {
      parent.appendChild(document.createTextNode(text.slice(start, to)))
    }
  }

  /**
   * Replace `element`'s contents with `text`, each matched range wrapped in
   * a `<mark>`. An empty `ranges` therefore restores the plain text, which
   * is how a cleared query undoes a highlight.
   * @param {HTMLElement} element
   * @param {string} text
   * @param {Span[]} ranges
   * @param {boolean} breakAfterUnderscore
   */
  function fill(element, text, ranges, breakAfterUnderscore) {
    while (element.firstChild) {
      element.removeChild(element.firstChild)
    }
    var cursor = 0
    for (var i = 0; i < ranges.length; i++) {
      var range = /** @type {Span} */ (ranges[i])
      appendRun(element, text, cursor, range[0], breakAfterUnderscore)
      var mark = document.createElement('mark')
      mark.className = 'match-highlight'
      appendRun(mark, text, range[0], range[1], breakAfterUnderscore)
      element.appendChild(mark)
      cursor = range[1]
    }
    appendRun(element, text, cursor, text.length, breakAfterUnderscore)
  }

  /**
   * Render a lint name with its matches highlighted, keeping the `<wbr>`
   * break opportunities the name needs in a narrow column.
   * @param {HTMLElement} element
   * @param {string} name
   * @param {Span[]} ranges
   */
  function renderName(element, name, ranges) {
    fill(element, name, ranges, true)
  }

  /**
   * Render prose with its matches highlighted.
   * @param {HTMLElement} element
   * @param {string} text
   * @param {Span[]} ranges
   */
  function renderText(element, text, ranges) {
    fill(element, text, ranges, false)
  }

  return {
    renderName: renderName,
    renderText: renderText,
  }
})()
