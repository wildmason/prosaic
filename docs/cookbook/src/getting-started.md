# Getting Started

Add the core engine and English grammar module to your project:

```toml
[dependencies]
prosaic-core = "0.1"
prosaic-grammar-en = "0.1"
```

## Your first render

```rust
use prosaic_core::{ctx, Engine, Session, Strictness, Variation};
use prosaic_grammar_en::English;

fn main() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);

    engine
        .register_template(
            "code.renamed",
            "{old_name} was renamed to {new_name}\
             {?consumer_count}, affecting {consumer_count} \
             direct {consumer_count|pluralize:consumer}{/?}",
        )
        .unwrap();

    let mut session = Session::new();

    let output = engine
        .render(
            &mut session,
            "code.renamed",
            &ctx! {
                old_name: "AuthHelper",
                new_name: "AuthService",
                consumer_count: 14,
            },
        )
        .unwrap();

    println!("{output}");
    // "AuthHelper was renamed to AuthService, affecting 14 direct consumers."
}
```

## The `ctx!` macro

`ctx!` builds a `Context` without the `Value::*` wrapper noise. It accepts
string literals, integer literals, owned `String`s, and `Vec<String>` lists:

```rust
use prosaic_core::{ctx, Value};

// These are equivalent:
let a = ctx! { name: "Foo", count: 3 };

let mut b = prosaic_core::Context::new();
b.insert("name", Value::String("Foo".into()));
b.insert("count", Value::Number(3));
```

Use `ctx!` in all new code. The verbose form is only needed when you need to
build a context programmatically (e.g. deserializing from JSON).

## The `Session` lifecycle

`Session` carries discourse state: what entities have been mentioned, what
words appeared recently, and which pronoun forms are in play. A single `Session`
should span a coherent block of related output — a document, a batch of alerts,
an incident narrative.

```rust
let mut session = Session::new();

// First render — "The class UserService was renamed to AuthService."
let s1 = engine.render(&mut session, "code.renamed", &ctx1).unwrap();

// Second render — discourse state allows pronouns: "It was also modified."
let s2 = engine.render(&mut session, "code.modified", &ctx2).unwrap();

// Between unrelated batches, reset to clear discourse state.
session.reset();

// Fresh paragraph — no pronoun carryover from previous block.
let s3 = engine.render(&mut session, "code.renamed", &ctx3).unwrap();
```

Resetting between documents prevents pronoun bleed ("it" referring to an
entity from the previous report).

## Variation modes

| Mode | Behaviour |
|------|-----------|
| `Variation::Fixed` | Always the first registered alternative. Deterministic. |
| `Variation::Seeded(u64)` | Hash-based selection — same seed + key = same index. |
| `Variation::RoundRobin` | Cycles through alternatives in registration order. |
| `Variation::Random` | Non-deterministic selection. |

`Fixed` is the right default for tests. `Seeded` is ideal for CI-generated
reports that need stable output. `Random` suits live applications where
phrasing variety matters over run-to-run consistency.
