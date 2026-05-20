# prosaic-vocab-release

Release-note vocabulary templates for Prosaic.

This crate registers templates for the lifecycle of a software release:
version tagging, features, breaking changes, bug fixes, security fixes,
deprecations, dependency updates, contributor summaries, release stats, and
overall summaries.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-en = "1.0.1"
prosaic-vocab-release = "1.0.1"
```

## Template Keys

- `release.tagged`
- `release.feature_added`
- `release.breaking_change`
- `release.bugfix`
- `release.security_fix`
- `release.deprecation`
- `release.dependency_update`
- `release.contributor_summary`
- `release.stats`
- `release.summary`

## Example

```rust
use prosaic_core::{Context, Engine, Session, Value};
use prosaic_grammar_en::English;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::new(English::new());
    prosaic_vocab_release::register(&mut engine)?;

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("README publishing".into()));
    ctx.insert(
        "description",
        Value::String("Each crate now ships package-specific documentation.".into()),
    );

    let mut session = Session::new();
    let text = engine.render(&mut session, "release.feature_added", &ctx)?;
    println!("{text}");
    Ok(())
}
```

Use `prosaic_vocab_release::register_es` with `prosaic-grammar-es` for Spanish
surface text.

## License

MIT OR Apache-2.0
