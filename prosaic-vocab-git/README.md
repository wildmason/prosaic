# prosaic-vocab-git

Git and VCS activity vocabulary templates for Prosaic.

This crate registers templates for commits, pull requests, issues, reviews, and
releases. It is useful for changelogs, activity digests, agent progress reports,
and CI/release narration.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-en = "1.0.1"
prosaic-vocab-git = "1.0.1"
```

## Template Keys

- `git.commit`
- `git.pr_opened`
- `git.pr_merged`
- `git.pr_closed`
- `git.issue_opened`
- `git.issue_closed`
- `git.review_approved`
- `git.review_changes_requested`
- `git.release`

## Example

```rust
use prosaic_core::{Context, Engine, Session, Value};
use prosaic_grammar_en::English;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::new(English::new());
    prosaic_vocab_git::register(&mut engine)?;

    let mut ctx = Context::new();
    ctx.insert("number", Value::Number(42));
    ctx.insert("title", Value::String("Add retry handling".into()));
    ctx.insert("author", Value::String("Alice".into()));
    ctx.insert("merger", Value::String("Bob".into()));

    let mut session = Session::new();
    let text = engine.render(&mut session, "git.pr_merged", &ctx)?;
    println!("{text}");
    Ok(())
}
```

Use `prosaic_vocab_git::register_es` with `prosaic-grammar-es` for Spanish
surface text.

## License

MIT OR Apache-2.0
