// ============================================================================
// Runs the cases in this directory under `node:test`, in a context with no
// DOM in it at all.
//
// The libraries under test are classic scripts that publish a global, so
// there is nothing to import: `node:vm` evaluates each file's source in one
// shared context, exactly as a browser evaluates a run of `<script>` tags,
// and the globals they declare land on that context rather than on this
// module's. Nothing is stubbed and nothing is shimmed. The context is bare
// — no `document`, no `window`, no `localStorage` — which is the point: a
// library that reached for one would fail here, loudly, instead of quietly
// becoming untestable. (A Rust test holds the same line by reading the
// sources; see `the_libraries_touch_no_dom`.)
//
// Node needs no packages for this. `node:test`, `node:vm` and the rest are
// built in, which is why `just test-js` does not install anything and why
// this file is not type-checked: tsconfig.json declares no ambient Node
// types, and pulling them in for one runner would mean a dev dependency to
// serve a file that CI runs on every change anyway.
//
// Every `*.test.js` beside this file is loaded, so a case file added here
// is picked up with no edit. The other runner — `tests.html`, rendered by
// tools/gen-docs/src/test_page.rs and shipped with the catalogue — loads a
// list it cannot glob, so a Rust test reads this directory and holds that
// list to it. That is what keeps the two runners running the same suite.
// ============================================================================

import { readdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { createContext, runInContext } from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const src = join(here, "..", "src");

const cases = (await readdir(here)).filter((name) => name.endsWith(".test.js")).sort();
if (cases.length === 0) throw new Error(`no *.test.js in ${here}`);

const context = createContext({});
const load = [join(src, "match.js"), join(src, "rank.js"), join(here, "harness.js")].concat(
  cases.map((name) => join(here, name)),
);
for (const file of load) {
  runInContext(await readFile(file, "utf8"), context, { filename: file });
}

for (const item of context.perfectionistTests.all()) {
  test(item.name, item.run);
}
