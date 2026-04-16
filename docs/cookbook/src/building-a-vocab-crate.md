# Building a Vocabulary Crate

Vocabulary crates bundle domain-specific templates and expose a single
`register` function. They're the standard unit of reuse across Prosaic
applications — the same pattern used by `prosaic-vocab-code`, `prosaic-vocab-git`,
and the other first-party crates.

## Scaffold

```sh
cargo new prosaic-vocab-myapp --lib
cd prosaic-vocab-myapp
```

`Cargo.toml`:

```toml
[package]
name = "prosaic-vocab-myapp"
version = "0.1.0"
edition = "2024"

[dependencies]
prosaic-core = "0.2"

[dev-dependencies]
prosaic-grammar-en = "0.2"
```

## Public API

Expose a single entry point that registers all templates into a caller-provided
engine. Return `ProsaicError` so callers can propagate parse failures cleanly:

```rust
// src/lib.rs
use prosaic_core::{Engine, ProsaicError};

pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    register_deploy_templates(engine)?;
    register_alert_templates(engine)?;
    Ok(())
}
```

## Namespace convention

Use `domain.event` keys. For multilingual future-proofing, keep all English
templates in a `templates::en` submodule even if you only have English today:

```
myapp.deploy_started
myapp.deploy_completed
myapp.rollback
myapp.alert_fired
```

This lets a future `prosaic-grammar-es` crate register `myapp.*` keys with
Spanish phrasing without touching the English module.

## A complete event type example

```rust
use prosaic_core::{Engine, ProsaicError, Salience};

fn register_deploy_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: terse — no strategy, no impact detail
    engine.register_template_at(
        "myapp.deploy_started",
        "Deployment of {service} {version} to {environment} started",
        Salience::Low,
    )?;

    // Medium: default — includes deployer, optional strategy
    engine.register_template(
        "myapp.deploy_started",
        "Deployment of {service} version {version} to {environment} \
         has been initiated by {deployer}\
         {?strategy} using {strategy} strategy{/?}",
    )?;

    // High: elaborative — for large fleet or critical environment
    engine.register_template_at(
        "myapp.deploy_started",
        "{deployer} initiated a {strategy|choose: blue-green=blue-green, \
         canary=canary, default=rolling} deployment of {service} {version} \
         to {environment} — a fleet of {instance_count|quantify} \
         {instance_count|pluralize:instance}",
        Salience::High,
    )?;

    Ok(())
}
```

## Template design tips

**Use conditional sections for optional slots.** Never require a slot that
won't always be present. Guard it with `{?slot}...{/?}` and the template
degrades gracefully when the information isn't available.

**Tier salience consistently across event types.** If your Low tier is always
terse and your High tier is always elaborative, callers get predictable
verbosity control without reading each template. Document your conventions in
the crate's README.

**Match pipe choices to content type.** Counts of things → `{n|pluralize:thing}`.
Large counts → `{n|quantify}`. Confidence values → `{score|hedge}`. Don't
use `words` (number-to-word) for anything above ~20 — "four hundred and
seventeen" is not more readable than "417".

## Testing

Write one test per event type, asserting on key content phrases:

```rust
#[cfg(test)]
mod tests {
    use prosaic_core::{ctx, Engine, Session, Strictness, Variation};
    use prosaic_grammar_en::English;

    fn test_engine() -> Engine {
        let mut e = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed);
        super::register(&mut e).unwrap();
        e
    }

    #[test]
    fn deploy_started_includes_service_and_version() {
        let engine = test_engine();
        let mut session = Session::new();
        let out = engine
            .render(
                &mut session,
                "myapp.deploy_started",
                &ctx! {
                    service: "billing-api",
                    version: "3.4.1",
                    environment: "production",
                    deployer: "alice",
                },
            )
            .unwrap();
        assert!(out.contains("billing-api"), "service name absent: {out}");
        assert!(out.contains("3.4.1"), "version absent: {out}");
        assert!(out.contains("production"), "environment absent: {out}");
    }

    #[test]
    fn deploy_started_omits_strategy_when_absent() {
        let engine = test_engine();
        let mut session = Session::new();
        let out = engine
            .render(
                &mut session,
                "myapp.deploy_started",
                &ctx! {
                    service: "billing-api",
                    version: "3.4.1",
                    environment: "staging",
                    deployer: "alice",
                    // no strategy key
                },
            )
            .unwrap();
        assert!(
            !out.contains("strategy"),
            "strategy clause should be absent: {out}"
        );
    }
}
```

Test every conditional branch, every salience tier, and failure modes like
an empty list or a zero count. Aim for one assertion per observable behaviour,
not one assertion per template.
