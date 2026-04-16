# Migrating from RosaeNLG

RosaeNLG is a JavaScript NLG library built on Pug templates. Prosaic covers
the same conceptual ground — template-based NLG with synonym variation,
referring expressions, and verb conjugation — but as a native Rust library
with no Node.js runtime requirement.

## Concept mapping

| RosaeNLG | Prosaic | Notes |
|----------|---------|-------|
| Pug mixin (`mixin myEvent(data)`) | `engine.register_template("key", "…")` | Templates are strings, not Pug AST nodes |
| `#[+value(data.name)]` | `{name}` | Direct slot substitution |
| `#[+value(data.name) \| upper]` | `{name\|capitalize}` | Pipe chains replace Pug filters |
| `synz` block | `{word\|syn}` pipe + `SynonymRegistry` | Register synonyms per slot, not inline |
| `#[+verb(data.action, {tense: "PAST"})]` | `{action\|verb:past}` | Tense is a pipe argument |
| `#[+subjectVerb(subject, verb)]` | `Sentence` / `Clause` builder API | Programmatic sentence assembly |
| `choosebest` (anti-repeat) | `Variation::Seeded` + discourse state | Discourse state is maintained by `Session` |
| `forEach` aggregation | `engine.render_batch(&mut session, &events)` | Clause reduction happens automatically |
| Referring expression (`#[+ref(entity)]`) | `{name\|refer}` pipe | REG algorithm is Dale-Reiter or graph-based |
| Multilingual `.pug` files | separate vocab crates per language | `prosaic-grammar-es` planned for v1.5 |

## Key differences

**No Node.js runtime.** Prosaic is a Rust library. Templates are parsed at
`register_template` call time and rendering allocates no heap beyond the output
string. There's no V8 startup cost, no npm install, no WASM bridge.

**Deterministic by default.** `Variation::Fixed` always picks the first
registered alternative. In RosaeNLG, synonym variation is non-deterministic
unless you seed the random generator explicitly. Prosaic's `Variation::Seeded`
gives you reproducibility with a single seed value.

**PARENT faithfulness built in.** Prosaic ships a reference-free faithfulness
scorer. There's no equivalent in RosaeNLG — hallucination checking was
left to the caller.

**Strictness is explicit.** RosaeNLG silently renders missing values as `null`.
Prosaic has three modes: `Strict` (error), `Lenient` (placeholder), `Silent`
(empty string). Set `Strict` in tests so missing slots surface immediately.

## Migrating templates

A RosaeNLG mixin like this:

```pug
mixin codeRenamed(data)
  | #{data.oldName}
  if data.consumerCount
    |  was renamed to #{data.newName},
    |  affecting #{data.consumerCount}
    if data.consumerCount == 1
      |  consumer
    else
      |  consumers
  else
    |  was renamed to #{data.newName}
```

Becomes this Prosaic template:

```rust
engine.register_template(
    "code.renamed",
    "{old_name} was renamed to {new_name}\
     {?consumer_count}, affecting {consumer_count} \
     {consumer_count|pluralize:consumer}{/?}",
).unwrap();
```

The conditional `{?consumer_count}...{/?}` handles both the zero and non-zero
case. The `pluralize` pipe handles the singular/plural split. The Pug
conditionals, string concatenation, and manual plural logic collapse into two
template features.

## Migrating synonym variation

RosaeNLG `synz` blocks list alternatives inline. Prosaic registers synonyms
separately into a `SynonymRegistry`:

```rust
// RosaeNLG
// synz
//   syn: was renamed to
//   syn: is now called
//   syn: has been renamed to

// Prosaic — register multiple templates under the same key
engine.register_template("code.renamed", "{old_name} was renamed to {new_name}").unwrap();
engine.register_template("code.renamed", "{old_name} is now called {new_name}").unwrap();
engine.register_template("code.renamed", "{old_name} has been renamed to {new_name}").unwrap();
```

The engine's `Variation` mode selects between them. `Seeded` gives you stable
output; `Random` or `RoundRobin` gives natural variety.

## What's not yet supported

**RosaeNLG's five languages.** RosaeNLG ships grammar support for English,
French, German, Italian, and Spanish. Prosaic is English-only in v1. Spanish
(`prosaic-grammar-es`) is planned for v1.5. Other languages will follow.

**Deep Pug template inheritance.** RosaeNLG templates can extend and include
other Pug files via the standard Pug inheritance chain. Prosaic supports
partials (`{>partial_name}`) for fragment reuse, but not deep template
hierarchies. This is rarely a limitation in practice — most NLG templates
are shallow.

**Arbitrary JavaScript in templates.** RosaeNLG templates can run arbitrary
JavaScript. Prosaic templates are sandboxed — they can only substitute values,
apply pipes, and branch on truthiness. This is a deliberate design choice:
sandboxed templates are safe to receive from untrusted sources and trivial to
serialize.
