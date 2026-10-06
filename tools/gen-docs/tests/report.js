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

  // Which engine produced the result below. The page's whole point is
  // that the answer differs between browsers, so a result that does not
  // say which one it came from is half an answer — and a screenshot of
  // one is unreportable.
  engine.textContent = navigator.userAgent;

  var cases = perfectionistTests.all();
  var failed = 0;

  for (var i = 0; i < cases.length; i++) {
    var failure = "";
    try {
      cases[i].run();
    } catch (thrown) {
      failure = thrown instanceof Error ? thrown.message : String(thrown);
      failed += 1;
    }
    var row = document.createElement("li");
    row.className = failure ? "fail" : "pass";
    var name = document.createElement("span");
    name.className = "name";
    name.textContent = cases[i].name;
    row.appendChild(name);
    if (failure) {
      var why = document.createElement("span");
      why.className = "why";
      why.textContent = failure;
      row.appendChild(why);
    }
    list.appendChild(row);
  }

  summary.textContent = failed
    ? failed + " of " + cases.length + " cases failed"
    : "all " + cases.length + " cases passed";
  summary.className = failed ? "fail" : "pass";
})();
