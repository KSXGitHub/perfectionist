// ============================================================================
// A registry and a handful of assertions, shared by the two ways these
// tests run.
//
// The page's libraries are classic scripts that publish a global, which is
// what makes them testable at all without a module system — and it is also
// what lets one set of cases serve both runners. A case registers itself
// here; `run.mjs` loads this file and the case files into a bare V8 context
// and hands each case to `node:test`, while `tests.html` loads the same
// files into a browser and reports them on the page. Neither runner knows
// anything about the other.
//
// The assertions are deliberately few. These cases check scores, orderings
// and ranges, so what is wanted is equality, deep equality on small arrays,
// and comparison — not a framework. Each throws an `Error` whose message
// says what was expected and what arrived, which is all either runner needs
// to report a failure.
// ============================================================================

var perfectionistTests = (function () {
  /** @type {{ name: string, run: () => void }[]} */
  var cases = [];

  /**
   * Register a case. The name is what both runners print, so it should
   * read as the claim being made rather than as a label.
   * @param {string} name
   * @param {() => void} run
   */
  function add(name, run) {
    cases.push({ name: name, run: run });
  }

  /** @returns {{ name: string, run: () => void }[]} */
  function all() {
    return cases;
  }

  /**
   * How a value is shown in a failure message. `JSON.stringify` returns
   * `undefined` — the value, not the string — for a function or an
   * undefined input, so fall back to `String`.
   * @param {unknown} value
   * @returns {string}
   */
  function show(value) {
    var text = JSON.stringify(value);
    return text === undefined ? String(value) : text;
  }

  /**
   * @param {unknown} value
   * @param {string} claim
   */
  function ok(value, claim) {
    if (value) return;
    throw new Error(claim + " — got " + show(value));
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
      throw new Error(claim + " — got " + show(value));
    }
    return value;
  }

  /**
   * @param {unknown} actual
   * @param {unknown} expected
   * @param {string} claim
   */
  function equal(actual, expected, claim) {
    if (actual === expected) return;
    throw new Error(claim + " — expected " + show(expected) + ", got " + show(actual));
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
    if (show(actual) === show(expected)) return;
    throw new Error(claim + " — expected " + show(expected) + ", got " + show(actual));
  }

  /**
   * @param {number} larger
   * @param {number} smaller
   * @param {string} claim
   */
  function greater(larger, smaller, claim) {
    if (larger > smaller) return;
    throw new Error(claim + " — expected " + show(larger) + " > " + show(smaller));
  }

  return {
    add: add,
    all: all,
    ok: ok,
    found: found,
    equal: equal,
    deepEqual: deepEqual,
    greater: greater,
  };
})();
