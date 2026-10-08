// ============================================================================
// Runs the cases in this directory, in a context with no DOM in it, and
// reports them the way `cargo test` reports its own.
//
// The libraries under test are classic scripts that publish a global, so
// there is nothing to import: `node:vm` evaluates each file's source in one
// shared context, exactly as a browser evaluates a run of `<script>` tags,
// and the globals they declare land on that context rather than on this
// module's. The context is bare, which is the point — a library that
// reached for the DOM would fail here loudly rather than quietly become
// untestable.
//
// The fixtures load ahead of the cases, since a case reads the fixtures it
// is built from as it is evaluated.
// ============================================================================
import { readdir, readFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createContext, runInContext } from 'node:vm'

const here = dirname(fileURLToPath(import.meta.url))
const src = join(here, '..', 'src')

const beside = await readdir(here)
const cases = beside.filter(name => name.endsWith('.test.js')).sort()
const fixtures = beside.filter(name => name.endsWith('.fixtures.js')).sort()
if (cases.length === 0) throw new Error(`no *.test.js in ${here}`)

// The catalogue's libraries, in load order: each publishes a global the
// next ones read.
const libraries = [
  'match_text.js',
  'match_score.js',
  'match_admit.js',
  'match_tiers.js',
  'match.js',
  'rank.js',
]

const context = createContext({})
const load = libraries
  .map(name => join(src, name))
  .concat([join(here, 'harness.js')])
  .concat(fixtures.map(name => join(here, name)))
  .concat(cases.map(name => join(here, name)))
for (const file of load) {
  runInContext(await readFile(file, 'utf8'), context, { filename: file })
}

/**
 * What a case threw, as one line. The harness builds its `Error` inside
 * the vm context, so it belongs to that realm and `instanceof Error` is
 * false here however ordinary it looks — hence the duck-type.
 * `String(thrown)` would answer too, but prefixes `Error: ` onto a
 * message already written to read on its own.
 */
function reason(thrown) {
  if (thrown !== null && typeof thrown === 'object' && 'message' in thrown) {
    return String(thrown.message)
  }
  return String(thrown)
}

for (const group of context.perfectionistTests.all()) {
  const started = process.hrtime.bigint()
  const failures = []

  console.log(`\n     Running ${group.name}\n`)
  console.log(`running ${group.cases.length} ${group.cases.length === 1 ? 'test' : 'tests'}`)
  for (const item of group.cases) {
    try {
      item.run()
      console.log(`test ${item.name} ... ok`)
    } catch (thrown) {
      failures.push({ name: item.name, why: reason(thrown) })
      console.log(`test ${item.name} ... FAILED`)
    }
  }

  if (failures.length > 0) {
    console.log('\nfailures:\n')
    for (const failure of failures) {
      console.log(`---- ${failure.name} ----`)
      console.log(`${failure.why}\n`)
    }
    console.log('failures:')
    for (const failure of failures) {
      console.log(`    ${failure.name}`)
    }
    process.exitCode = 1
  }

  const seconds = Number(process.hrtime.bigint() - started) / 1e9
  console.log(
    `\ntest result: ${failures.length === 0 ? 'ok' : 'FAILED'}. ` +
      `${group.cases.length - failures.length} passed; ${failures.length} failed; ` +
      `finished in ${seconds.toFixed(2)}s`,
  )
}
console.log()
