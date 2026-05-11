# Monitoring Alerts to Prose

`prosaic-tracing` is a [`tracing-subscriber`](https://docs.rs/tracing-subscriber)
`Layer` that intercepts tracing events and renders them as prose via a registered
`Engine`. Drop it into your existing subscriber stack with no changes to call
sites.

## Add the dependency

```toml
[dependencies]
prosaic-core = "1.0.0"
prosaic-grammar-en = "1.0.0"
prosaic-tracing = "1.0.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["registry"] }
```

## Register templates for your alert keys

The layer matches events using a key derived from the event's `target` and
`name` fields: `"{target}.{name}"`. Register a template under that key and the
layer renders it whenever a matching event fires.

```rust
use prosaic_core::{Engine, Strictness, Variation};
use prosaic_grammar_en::English;
use prosaic_tracing::ProsaicLayer;
use tracing_subscriber::prelude::*;

fn build_prose_layer(log_file: std::fs::File) -> ProsaicLayer<std::fs::File> {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent) // skip events with missing slots
        .variation(Variation::Seeded(42));

    engine
        .register_template(
            "http.threshold_breached",
            "The {metric} on {service} has exceeded its threshold, \
             reaching {current_value} against a limit of {threshold}\
             {?duration} sustained for {duration}{/?}",
        )
        .unwrap();

    engine
        .register_template(
            "http.error_spike",
            "{service} is experiencing {error_count|quantify} errors per \
             minute{?error_type} of type {error_type}{/?}",
        )
        .unwrap();

    engine
        .register_template(
            "http.recovery",
            "{service} has recovered from {incident_type}. \
             All health checks are now passing",
        )
        .unwrap();

    ProsaicLayer::new(engine, log_file)
}
```

## Wire it into your subscriber

```rust
fn main() {
    let prose_log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("alerts.prose.log")
        .expect("open prose log");

    let prose_layer = build_prose_layer(prose_log);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer()) // keep the existing formatter
        .with(prose_layer)                       // add prose layer on top
        .init();

    // From here on, matching tracing events produce lines in alerts.prose.log
    run_server();
}
```

## Emit events from your handlers

```rust
async fn checkout_handler(req: Request) -> Response {
    let start = std::time::Instant::now();

    match process_checkout(req).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!(
                name: "error_spike",
                target: "http",
                service = "checkout-service",
                error_count = 1i64,
                error_type = e.kind(),
            );
            Response::internal_error()
        }
    }
}
```

The layer sees `target = "http"`, `name = "error_spike"` and derives key
`"http.error_spike"`. It builds a `Context` from the event fields and renders
the registered template. Events with no matching template are silently skipped.

## Custom key mapper

If your event naming convention doesn't match `"{target}.{name}"`:

```rust
let layer = ProsaicLayer::new(engine, writer)
    .key_mapper(|meta| {
        // Use only the name, ignoring target
        meta.name().to_string()
    });
```

The mapper receives a `&tracing::Metadata<'_>` and returns a `String` key.

## When to use this vs. a plain log formatter

Use `ProsaicLayer` when:
- Alert output will be read by humans (on-call engineers, Slack channels,
  incident tickets) and grammatical English matters more than machine parse-ability.
- You want prose that adapts to context — hedging language when confidence is
  low, condensing multi-field alerts into a single fluent sentence.

Stick with a plain formatter when:
- The output feeds another machine (log aggregator, metrics pipeline).
- Every field must be individually queryable by key.
- You need sub-microsecond overhead on the hot path.

`ProsaicLayer` adds rendering overhead per matching event. Non-matching events
(no registered template) have negligible overhead — just a hash lookup.
