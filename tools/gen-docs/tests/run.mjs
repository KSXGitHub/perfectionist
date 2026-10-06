// ============================================================================
// Runs the cases in this directory, in a context with no DOM in it at all,
// and reports them the way `cargo test` reports its own.
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
// Node needs no packages for this: `node:vm` and the rest are built in,
// which is why `just test-js` installs nothing. Nor is this file
// type-checked — tsconfig.json declares no ambient Node types, and pulling
// them in for one runner would mean a dev dependency to serve one file
// that CI runs on every change regardless.
//
// `node:test` ran these until the output became the problem: six lines a
// case in TAP, 214 of them for thirty-four cases, behind a banner naming
// a protocol nothing here speaks. Its other reporters trade that for a
// tick and a duration per case, or for rows of dots. What it was doing
// for us was a registry, an exit code and a try/catch, and the harness is
// already the registry — so the rest is written out below, in the shape
// `cargo test` reports its own, which is the shape a reader of this
// repository already knows how to read.
//
// Every `*.test.js` beside this file is loaded, so a case file added here
// is picked up with no edit. The other runner — `tests.html`, rendered by
// tools/gen-docs/src/test_page.rs and shipped with the catalogue — loads a
// list it cannot glob, so a Rust test reads this directory and holds that
// list to it. That is what keeps the two runners running the same suite.
// ============================================================================

import { readdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
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

/**
 * What a case threw, as one line. The harness throws an `Error` built
 * inside the vm context, so it is an `Error` of *that* realm and
 * `instanceof Error` here is false however ordinary it looks — hence the
 * duck-type. `String(thrown)` would answer too, but with an `Error: `
 * prefix in front of a message already written to read on its own.
 */
function reason(thrown) {
  if (thrown !== null && typeof thrown === "object" && "message" in thrown) {
    return String(thrown.message);
  }
  return String(thrown);
}

for (const group of context.perfectionistTests.all()) {
  const started = process.hrtime.bigint();
  const failures = [];

  console.log(`\n     Running ${group.name}\n`);
  console.log(`running ${group.cases.length} ${group.cases.length === 1 ? "test" : "tests"}`);
  for (const item of group.cases) {
    try {
      item.run();
      console.log(`test ${item.name} ... ok`);
    } catch (thrown) {
      failures.push({ name: item.name, why: reason(thrown) });
      console.log(`test ${item.name} ... FAILED`);
    }
  }

  if (failures.length > 0) {
    console.log("\nfailures:\n");
    for (const failure of failures) {
      console.log(`---- ${failure.name} ----`);
      console.log(`${failure.why}\n`);
    }
    console.log("failures:");
    for (const failure of failures) {
      console.log(`    ${failure.name}`);
    }
    process.exitCode = 1;
  }

  const seconds = Number(process.hrtime.bigint() - started) / 1e9;
  console.log(
    `\ntest result: ${failures.length === 0 ? "ok" : "FAILED"}. ` +
      `${group.cases.length - failures.length} passed; ${failures.length} failed; ` +
      `finished in ${seconds.toFixed(2)}s`,
  );
}
console.log();
