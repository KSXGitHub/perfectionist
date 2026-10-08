// ============================================================================
// A registry and a handful of assertions, shared by the two ways these
// tests run.
//
// The page's libraries are classic scripts that publish a global, which is
// what makes them testable at all without a module system — and it is also
// what lets one set of cases serve both runners. A case registers itself
// here, against the group its file opened; `run.mjs` loads this file and
// the case files into a bare V8 context and runs each group from the
// terminal, while `tests.html` loads the same files into a browser and
// reports them on the page. Neither runner knows anything about the other.
//
// The assertions are deliberately few. These cases check scores, orderings
// and ranges, so what is wanted is equality, deep equality on small arrays,
// and comparison — not a framework. Each throws an `Error` whose message
// says what was expected and what arrived, which is all either runner needs
// to report a failure.
//
// What a case should assert, wherever it can, is an ordering or a range
// rather than an exact score. A score pins the arithmetic, which is the
// part most likely to be deliberately changed — the weights behind it were
// chosen by eye and are expected to be re-tuned — whereas "a contiguous
// match beats a scattered one" is the property that has to survive any
// re-tuning. Where a number is asserted it is a threshold relationship,
// not a literal.
// ============================================================================

/**
 * One case: the claim it makes, and the function that checks it.
 * @typedef {object} TestCase
 * @property {string} name
 * @property {() => void} run
 */

/**
 * One file's cases, which both runners report as a block.
 * @typedef {object} TestGroup
 * @property {string} name
 * @property {TestCase[]} cases
 */

var perfectionistTests = (function () {
  /** @type {TestGroup[]} */
  var groups = []

  /**
   * Open a group, which a case file does once at the top with its own
   * name. Both runners report a group at a time, the way `cargo test`
   * reports a test target at a time.
   * @param {string} name
   */
  function group(name) {
    groups.push({ name: name, cases: [] })
  }

  /**
   * Register a case against the open group. The name is what both
   * runners print, so it should read as the claim being made rather than
   * as a label.
   * @param {string} name
   * @param {() => void} run
   */
  function add(name, run) {
    if (groups.length === 0) {
      throw new Error('`' + name + '` was registered before any group was opened')
    }
    groups[groups.length - 1].cases.push({ name: name, run: run })
  }

  /** @returns {TestGroup[]} */
  function all() {
    return groups
  }

  /**
   * How a value is shown in a failure message. `JSON.stringify` returns
   * `undefined` — the value, not the string — for a function or an
   * undefined input, so fall back to `String`.
   * @param {unknown} value
   * @returns {string}
   */
  function show(value) {
    var text = JSON.stringify(value)
    return text === undefined ? String(value) : text
  }

  /**
   * @param {unknown} value
   * @param {string} claim
   */
  function ok(value, claim) {
    if (value) return
    throw new Error(claim + ' — got ' + show(value))
  }

  /**
   * The value, having asserted that it is there. Both matchers return
   * `null` for a non-match, so a case that goes on to read the score or
   * the ranges has to rule that out first — and reading a field off a
   * possibly-null value is a type error, which is why this hands the
   * value back narrowed rather than merely checking it.
   * @template T
   * @param {T | null | undefined} value
   * @param {string} claim
   * @returns {T}
   */
  function found(value, claim) {
    if (value === null || value === undefined) {
      throw new Error(claim + ' — got ' + show(value))
    }
    return value
  }

  /**
   * @param {unknown} actual
   * @param {unknown} expected
   * @param {string} claim
   */
  function equal(actual, expected, claim) {
    if (actual === expected) return
    throw new Error(claim + ' — expected ' + show(expected) + ', got ' + show(actual))
  }

  /**
   * Structural equality, by serialisation. Adequate because everything
   * compared here is a small array of numbers or strings; it would be
   * wrong for anything holding a function, a cycle or a `NaN`.
   * @param {unknown} actual
   * @param {unknown} expected
   * @param {string} claim
   */
  function deepEqual(actual, expected, claim) {
    if (show(actual) === show(expected)) return
    throw new Error(claim + ' — expected ' + show(expected) + ', got ' + show(actual))
  }

  /**
   * @param {number} larger
   * @param {number} smaller
   * @param {string} claim
   */
  function greater(larger, smaller, claim) {
    if (larger > smaller) return
    throw new Error(claim + ' — expected ' + show(larger) + ' > ' + show(smaller))
  }

  return {
    group: group,
    add: add,
    all: all,
    ok: ok,
    found: found,
    equal: equal,
    deepEqual: deepEqual,
    greater: greater,
  }
})()
