# prosaic-tracing

Bridge `tracing` events to natural language prose through Prosaic.

`prosaic-tracing` provides a `tracing_subscriber::Layer` that maps tracing event
metadata to Prosaic template keys, converts event fields into a Prosaic context,
and writes rendered prose to a configured writer.

## Install

```toml
[dependencies]
prosaic-core = "1.0.1"
prosaic-grammar-en = "1.0.1"
prosaic-tracing = "1.0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["registry"] }
```

## Example

```rust
use prosaic_core::{Engine, Strictness};
use prosaic_grammar_en::English;
use prosaic_tracing::ProsaicLayer;
use tracing_subscriber::prelude::*;

fn main() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Silent);
    engine
        .register_template(
            "auth.login_failed",
            "User {user_id} failed to log in after {attempts} attempts.",
        )
        .unwrap();

    let layer = ProsaicLayer::new(engine, std::io::stdout());
    let subscriber = tracing_subscriber::registry().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(
            name: "login_failed",
            target: "auth",
            user_id = 42,
            attempts = 5,
        );
    });
}
```

The default key mapper is `{target}.{event_name}`. Use `key_mapper` to adapt the
mapping when your tracing targets or event names do not match your template
namespace.

## License

MIT OR Apache-2.0
