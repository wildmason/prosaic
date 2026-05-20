# prosaic-wasm

WebAssembly bindings for the Prosaic NLG engine.

This crate exposes an English Prosaic engine and per-document sessions to
JavaScript via `wasm-bindgen`. Context values are plain JavaScript objects whose
values are strings, numbers, or arrays of strings.

## Install

```toml
[dependencies]
prosaic-wasm = "1.0.1"
```

Build the package for `wasm32-unknown-unknown` with `wasm-pack` or your existing
`wasm-bindgen` pipeline.

## JavaScript Example

```js
import init, { ProsaicEngine, ProsaicSession } from "./prosaic_wasm.js";

await init();

const engine = new ProsaicEngine();
engine.registerTemplate("hello", "Hello, {name}!");

const session = new ProsaicSession();
const text = engine.render(session, "hello", { name: "world" });
console.log(text);
```

## API Surface

- `registerTemplate(key, body)` and `registerTemplateWithStyle(key, body, style)`.
- `render(session, key, context)` for one event.
- `renderBatch(session, events)` for an array of `[key, context]` pairs.
- `loadManifest(json)` for bundles produced by `prosaic build --target=json`.
- `renderExplained`, `scoreVariants`, `scoreFaithfulness`, and
  `validateTemplate` for editor and debugging workflows.
- `setLanguagePreference` and `setStylePreference` for variant selection.

The crate disables Prosaic's `time` feature by default because
`wasm32-unknown-unknown` does not provide `SystemTime::now()`.

## License

MIT OR Apache-2.0
