# prosaic-vocab-pr

Pull-request narrative vocabulary templates for Prosaic.

Where `prosaic-vocab-git` covers discrete git events, this crate covers PR-level
state: review posture, scope, diff stats, age, merge readiness, CI status, and
relationships between pull requests.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-en = "1.0.1"
prosaic-vocab-pr = "1.0.1"
```

## Template Keys

- `pr.summary`
- `pr.review_state`
- `pr.scope`
- `pr.diff_stats`
- `pr.age`
- `pr.merge_readiness`
- `pr.ci_status`
- `pr.related_prs`

## Example

```rust
use prosaic_core::{Context, Engine, Session, Value};
use prosaic_grammar_en::English;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::new(English::new());
    prosaic_vocab_pr::register(&mut engine)?;

    let mut ctx = Context::new();
    ctx.insert("number", Value::Number(42));
    ctx.insert("approvals", Value::Number(2));
    ctx.insert("pending", Value::Number(1));

    let mut session = Session::new();
    let text = engine.render(&mut session, "pr.review_state", &ctx)?;
    println!("{text}");
    Ok(())
}
```

Use `prosaic_vocab_pr::register_es` with `prosaic-grammar-es` for Spanish
surface text.

## License

MIT OR Apache-2.0
