// ============================================================================
// Reports the cases on the page, for whoever is reading index.html in a
// browser.
//
// This is the second of the two runners, and the reason it exists is that
// the first one proves nothing about the engines the catalogue targets. The
// libraries under test ship to browsers years older than the Node that runs
// them in CI, and nothing in `run.mjs` would catch a construct V8 accepts
// today and an older engine rejects outright. Opening this page in one of
// those engines does.
//
// It is the only file here that touches the DOM, and it is not part of the
// generated site: the page is opened from the checkout over `file://`,
// loading the libraries straight out of `../src/`. Nothing about the tests
// is published.
// ============================================================================

(function () {
  var list = /** @type {HTMLElement} */ (document.querySelector("#cases"));
  var summary = /** @type {HTMLElement} */ (document.querySelector("#summary"));
  if (!list || !summary) return;

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
