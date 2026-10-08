// ============================================================================
// Reports the cases on the page, for whoever is reading `tests.html` in a
// browser.
//
// This is the second of the two runners, and the reason it exists is that
// the first one proves nothing about the engines the catalogue targets. The
// libraries under test ship to browsers years older than the Node that runs
// them in CI, and nothing in `run.mjs` would catch a construct V8 accepts
// today and an older engine rejects outright. Opening this page in one of
// those engines does — which is why the page ships with the catalogue
// rather than waiting in a checkout: an old device can be pointed at a URL.
//
// It is the only file here that touches the DOM. The page around it is
// rendered by tools/gen-docs/src/test_page.rs, where the rest of the site's
// markup lives.
// ============================================================================

(function () {
  var list = /** @type {HTMLElement} */ (document.querySelector("#cases"));
  var summary = /** @type {HTMLElement} */ (document.querySelector("#summary"));
  var engine = /** @type {HTMLElement} */ (document.querySelector("#engine"));
  if (!list || !summary || !engine) return;

  // The result below means little without the engine that produced it.
  engine.textContent = navigator.userAgent;

  // The glyph each state shows before a case's name. Text symbols rather
  // than emoji, for these reasons. They take `currentcolor`, so the shape
  // and the colour of a state cannot disagree, and a reader who sees
  // neither colour still has the shape. And this page exists to be opened
  // on engines old enough that an emoji font is the thing most likely to
  // be missing, where every one of these would come out as a blank box.
  var MARKS = {
    untested: "\u25CB",
    passed: "\u2713",
    failed: "\u2717",
  };

  /**
   * Put a row into one of its states: the attribute tests.css reads, the
   * glyph a reader sees, and the name a screen reader hears in its place
   * — the state is carried by each of them rather than by a class that
   * only CSS understands.
   * @param {HTMLElement} row
   * @param {HTMLElement} indicator
   * @param {"untested" | "passed" | "failed"} state
   */
  function setState(row, indicator, state) {
    row.setAttribute("data-result", state);
    indicator.textContent = MARKS[state];
    indicator.setAttribute("aria-label", state);
  }

  /**
   * What a case threw, as one line. Duck-typed rather than tested with
   * `instanceof`, which is how `run.mjs` reads it too: the two runners
   * report the same suite, so a throw has to read alike in both.
   * @param {unknown} thrown
   * @returns {string}
   */
  function reason(thrown) {
    if (thrown !== null && typeof thrown === "object" && "message" in thrown) {
      return String(thrown.message);
    }
    return String(thrown);
  }

  var groups = perfectionistTests.all();
  var total = 0;
  var failed = 0;

  for (var g = 0; g < groups.length; g++) {
    var heading = document.createElement("h2");
    heading.textContent = groups[g].name;
    list.appendChild(heading);

    var rows = document.createElement("ul");
    var cases = groups[g].cases;
    total += cases.length;

    // Every row goes in untested, and is run afterwards. Nothing is
    // painted in between, so the state is not there to be watched — it is
    // there for when the reporter itself throws part way through, which
    // leaves the rows past that point reading `untested` on screen
    // instead of never appearing at all.
    var pending = [];
    for (var i = 0; i < cases.length; i++) {
      var row = document.createElement("li");
      var indicator = document.createElement("span");
      indicator.className = "result";
      indicator.setAttribute("role", "img");
      setState(row, indicator, "untested");
      row.appendChild(indicator);
      var name = document.createElement("span");
      name.className = "name";
      name.textContent = cases[i].name;
      row.appendChild(name);
      rows.appendChild(row);
      pending.push({ run: cases[i].run, row: row, indicator: indicator });
    }
    list.appendChild(rows);

    for (var j = 0; j < pending.length; j++) {
      // Whether it threw, kept apart from what it said: a throw carrying
      // no message would otherwise read as having passed while still
      // counting against the total, and the summary would contradict
      // every row on the page.
      var broke = false;
      var failure = "";
      try {
        pending[j].run();
      } catch (thrown) {
        broke = true;
        failure = reason(thrown);
        failed += 1;
      }
      setState(pending[j].row, pending[j].indicator, broke ? "failed" : "passed");
      if (!failure) continue;
      // The harness's message, quoted verbatim: output from a program,
      // which is what `samp` is for.
      var why = document.createElement("samp");
      why.className = "why";
      why.textContent = failure;
      pending[j].row.appendChild(why);
    }
  }

  // A case file an engine rejects outright — the failure this page exists
  // to find — is skipped whole, and registers nothing. The groups that did
  // register would then report green for a suite that had silently shrunk,
  // so the page carries how many files it loaded and this holds the
  // registered groups to that count. A missing attribute reads as 0 and
  // asks for nothing; a test beside test_page.rs keeps it there.
  var expected = Number(list.getAttribute("data-expected-groups"));
  var missing = expected > groups.length ? expected - groups.length : 0;
  var verdict = [];
  if (missing > 0) {
    verdict.push(missing + " of " + expected + " case files did not load");
  }
  if (failed > 0) {
    verdict.push(failed + " of " + total + " cases failed");
  }
  summary.textContent = verdict.length > 0 ? verdict.join("; ") : "all " + total + " cases passed";
  summary.setAttribute("data-result", verdict.length > 0 ? "failed" : "passed");
})();
