# prosaic-vocab-code

Code-analysis vocabulary templates for Prosaic.

This crate registers ready-made templates for common code change events, with
English and Spanish template sets exposed through matching registration
functions.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-en = "1.0.1"
prosaic-vocab-code = "1.0.1"
```

## Template Keys

- `code.renamed`
- `code.deleted`
- `code.added`
- `code.modified`
- `code.moved`
- `code.signature_changed`

Each key has multiple salience variants so the engine can render terse low
impact notes or more detailed high impact prose from the same event shape.

## Example

```rust
use prosaic_core::{Context, Engine, Session, Value};
use prosaic_grammar_en::English;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::new(English::new());
    prosaic_vocab_code::register(&mut engine)?;

    let mut ctx = Context::new();
    ctx.insert("entity_type", Value::String("class".into()));
    ctx.insert("old_name", Value::String("UserService".into()));
    ctx.insert("new_name", Value::String("AccountService".into()));
    ctx.insert("consumer_count", Value::Number(3));

    let mut session = Session::new();
    let text = engine.render(&mut session, "code.renamed", &ctx)?;
    println!("{text}");
    Ok(())
}
```

Use `prosaic_vocab_code::register_es` with `prosaic-grammar-es` for Spanish
surface text.

## License

MIT OR Apache-2.0
