# Template Authoring Guide

Templates are strings with slot expressions embedded in curly braces. The
parser is invoked once at registration time; rendering is cheap.

## Slot syntax

```
{key}                 — substitute the value of key
{key|pipe}            — apply a transform
{key|pipe:arg}        — pipe with an argument
{key|pipe1|pipe2}     — chained pipes (left to right)
{key|pipe1|pipe2:arg} — chain with argument on the last pipe
```

Slot keys map directly to context keys. If a key is absent and the engine is
in `Strictness::Strict` mode, rendering returns an error. With `Lenient` it
renders `[missing: key]`; with `Silent` it renders an empty string.

## Conditional sections

```
{?key}...{/?}
```

The inner content renders only when `key` is _truthy_: a non-empty string,
a non-zero number, or a non-empty list. A missing key counts as falsy —
conditionals never error on absent keys regardless of strictness mode.

Conditionals nest and can contain other slots and pipes:

```
{service} has recovered{?downtime_minutes} after {downtime_minutes} \
{downtime_minutes|pluralize:minute} of degradation{/?}
```

If `downtime_minutes` is absent or zero, the output is just
`"{service} has recovered"`.

## Partials

```
{>partial_name}
```

Register reusable fragments with `engine.register_partial`:

```rust
engine.register_partial(
    "consumer_clause",
    ", affecting {consumer_count} {consumer_count|pluralize:consumer}",
).unwrap();

engine.register_template(
    "code.renamed",
    "{old_name} was renamed to {new_name}{?consumer_count}{>consumer_clause}{/?}",
).unwrap();
```

Partials keep repeated clauses consistent across many templates and reduce
the surface area for copy-paste drift.

## Pipe reference

| Pipe | Argument | Input | Output | Example |
|------|----------|-------|--------|---------|
| `pluralize` | singular noun | `Number` | inflected noun | `{n\|pluralize:item}` → `"3 items"` |
| `article` | — | `String` | noun with article | `{entity_type\|article}` → `"an API"` |
| `join` | style (optional) | `List` | joined string | `{names\|join}` → `"A, B, and C"` |
| `truncate` | max count | `List` | shortened list | `{names\|truncate:3}` → first 3 + "…" |
| `capitalize` | — | `String` | title-cased | `{action\|capitalize}` → `"Renamed"` |
| `ordinal` | — | `Number` | ordinal string | `{rank\|ordinal}` → `"3rd"` |
| `words` | — | `Number` | word form | `{n\|words}` → `"twelve"` |
| `choose` | `k=v,…,default=v` | `String` | mapped value | `{sev\|choose: critical=alert, default=info}` |
| `refer` | — | `String` | referring expression | `{name\|refer}` → `"The class Foo"` / `"it"` |
| `verb` | tense/form | `String` | conjugated verb | `{action\|verb:past}` → `"renamed"` |
| `syn` | — | `String` | synonym variant | `{word\|syn}` → varies by SynonymRegistry |
| `quantify` | mode (optional) | `Number` | quantified phrase | `{n\|quantify}` → `"thousands of"` |
| `hedge` | mode (optional) | `Number` | hedged phrase | `{conf\|hedge}` → `"likely"` |
| `demonstrative` | — | `String` | demonstrative | `{entity_type\|demonstrative}` → `"this class"` |
| `negated` | — | `String` | negated phrase | `{action\|negated}` → `"was not renamed"` |
| `relative` | — | `Number` (Unix ms) | time phrase | `{ts\|relative}` → `"3 minutes ago"` |

### `join` styles

The `join` pipe accepts an optional style argument:

```
{names|join}              — Oxford comma: "A, B, and C"
{names|join:bare}         — no Oxford comma: "A, B and C"
{names|join:bracketed}    — parenthetical: "including A, B, and C"
{names|join:or}           — disjunction: "A, B, or C"
```

**Join-style gotcha.** When the template text immediately before a list already
has a framing word, use `join:bracketed` to prevent doubling:

```
# WRONG — produces "impacting including A, B, and C"
"...impacting {endpoints|join}"

# RIGHT — produces "impacting (A, B, and C)"
"...impacting {endpoints|join:bracketed}"
```

Or drop the framing word entirely and let the pipe provide it:

```
# Also fine — produces "impacting A, B, and C"
"...impacting {endpoints|truncate:3|join:bare}"
```

### `choose` dispatch

`choose` maps a string value to a replacement. Keys are matched literally;
`default` fires when no key matches:

```
{severity|choose: critical=has critically exceeded, warning=has exceeded, default=is approaching}
```

Input `"critical"` → `"has critically exceeded"`.
Input `"info"` → `"is approaching"` (default).
Input absent → empty string (if Silent) or error (if Strict).

### `verb` tense forms

```
{action|verb:past}        — past tense ("renamed")
{action|verb:present}     — present tense ("renames")
{action|verb:progressive}  — progressive ("renaming")
{action|verb:infinitive}  — bare infinitive ("rename")
```

### `quantify` thresholds

`quantify` converts a large integer to a human-readable magnitude phrase,
leaving small counts as plain digits:

```
{n|quantify}   — n=4 → "4", n=1200 → "over a thousand", n=3500000 → "millions of"
```

## Salience-tiered alternatives

Register multiple templates under the same key at different salience levels
to match verbosity to event magnitude:

```rust
use prosaic_core::Salience;

// Terse — for low-impact events
engine.register_template_at(
    "code.renamed",
    "{old_name} was renamed to {new_name}",
    Salience::Low,
)?;

// Default — includes impact details
engine.register_template(
    "code.renamed",
    "{old_name} was renamed to {new_name}\
     {?consumer_count}, affecting {consumer_count} \
     {consumer_count|pluralize:consumer}{/?}",
)?;

// Elaborative — for high-impact events
engine.register_template_at(
    "code.renamed",
    "{old_name} has been renamed to {new_name} — a significant change \
     rippling through {consumer_count} {consumer_count|pluralize:consumer}",
    Salience::High,
)?;
```

The engine derives salience from context automatically: set a `"salience"`
key to `"low"`, `"medium"`, or `"high"`, or rely on `consumer_count`
thresholds (0–1 → Low, 2–19 → Medium, 20+ → High).

## Template design tips

**Keep conditionals narrow.** A single `{?key}...{/?}` wrapping an optional
clause is clean. Deeply nested conditionals are hard to reason about and
rarely needed — prefer registering separate templates at different salience
levels.

**Prefer `truncate` before `join` on unbounded lists.** Without `truncate`,
a list of 50 endpoints becomes a 200-character sentence. Set a reasonable
cap and let the `…` signal truncation:

```
{endpoints|truncate:3|join}
```

**Template literals contribute to faithfulness scoring.** Every word you put
in a template's literal text is in the entailment source set, so boilerplate
like "was renamed to" or "affecting" never triggers a hallucination flag.
Avoid putting content words in literals that aren't in context values — that's
not a hallucination, but it is invisible to faithfulness.

**Use `register_partial` for shared clauses.** Any clause that appears
verbatim in more than two templates should be a partial. It keeps the
entailment surface consistent and makes future edits a single-point change.
