# Implementation conventions

Conventions shared across multiple rules in this catalogue. Each rule
is otherwise self-contained; this file exists so the recurring
guidance lives in exactly one place.

## Parser style

When a rule needs to parse a non-trivial string — a URL, an email
address, a markdown span, a serde-attribute type literal, a
`#[derive(...)]` list — **prefer parser combinators over regex-style
matching**. Implement the scanner as a collection of small `take_*`
functions, each one consuming a prefix of its input and returning the
remainder.

The canonical shapes are:

```rust
// Always succeeds. Used for parsers that consume zero or more bytes
// of a known shape (skipping whitespace, taking a run of digits when
// "no digits" is a valid result, etc.).
fn take_whitespace(input: &str) -> ((), &str);

// Optional match. Used when the parser may not apply at this
// position; `None` lets the caller try a different alternative.
fn take_url_scheme(input: &str) -> Option<(UrlScheme, &str)>;

// Fallible match. Used when the parser commits to consuming the
// input but the input may be malformed. The `Error` carries enough
// context for a useful diagnostic.
fn take_email_local_part(input: &str) -> Result<(Cow<'_, str>, &str), EmailParseError>;
```

The pattern is well-trodden — `nom`, `chumsky`, `combine`, and
`winnow` all expose the same essential shape. Within `perfectionist`
these helpers are implemented by hand to avoid pulling a parser
library through the Dylint pass; the resulting code is small,
inspectable, and free of regex backtracking surprises.

### Why this style

- **Composition is explicit.** A complex parser is a sequence of
  small calls, each returning the remainder. The reader follows the
  data flow without skimming a regex line for capture-group offsets.
- **Each step is testable in isolation.** Unit tests sit next to the
  individual `take_*` function, exercise its corner cases, and run
  in microseconds.
- **No regex dependency.** Dylint passes load into the compiler
  process; every transitive crate is paid for at lint time. A regex
  engine is a poor fit for the small, fixed grammars these rules
  match.
- **Trailing-punctuation handling falls out naturally.** The
  caller's choice of when to commit to a match is visible in the
  call graph rather than buried in a non-greedy quantifier.
- **Span construction stays accurate.** Each combinator knows
  exactly how many bytes it consumed; converting that into a `Span`
  on the source map is a one-liner per call site.

### What to avoid

- Pulling in `regex`, `regex_lite`, or `aho-corasick` for what is
  fundamentally a small lexer. These crates are excellent in their
  niche; that niche is not a Dylint pass with a five-line grammar.
- Cramming a multi-step parser into a single `chars().filter().fold()`
  chain. The fold loses intermediate state and obscures where the
  failure points are.
- Returning byte offsets without the slice. The combinator return
  type bundles `(Parsed, &str)` so the caller cannot accidentally
  forget to advance.

### Where to draw the line

Trivial scans — locating one Unicode codepoint, counting lines,
matching a literal three-byte UTF-8 sequence — do not need a
combinator scaffold. Reach for `&[u8]::iter()` or
`memchr::memchr` directly. The combinator style is for rules whose
grammar has more than one alternative, more than one position-
dependent decision, or any kind of optional / repeated structure.

The rules that explicitly call this convention out in their
implementation notes are the candidates; rules that just walk a
fixed-size byte sequence do not.
