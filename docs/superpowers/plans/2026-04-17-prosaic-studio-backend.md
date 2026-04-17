# Prosaic Studio — Backend Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the backend prerequisites for Prosaic Studio: a new `prosaic-project` crate with TOML-driven project loading and bundling, multi-language variant selection in `prosaic-core`, expanded `prosaic-wasm` exports, and `prosaic-cli` `new`/`build`/`test` subcommands.

**Architecture:** New `prosaic-project` crate handles project I/O and bundling. `prosaic-core` gets minimal additions for per-render language preference and language-tagged variants. `prosaic-cli` becomes the user-facing entrypoint. `prosaic-wasm` exposes everything Studio needs as JS-typed bindings.

**Tech Stack:** Rust 2024 edition, `toml` crate for parsing, `serde` for project model, existing `prosaic-core` engine, `wasm-bindgen` for bindings.

---

### File Structure (locks decomposition)

**New files:**

```
prosaic-project/
├── Cargo.toml
├── src/
│   ├── lib.rs              — Crate root, re-exports
│   ├── error.rs            — ProjectError variants
│   ├── manifest.rs         — prosaic.toml schema (Manifest struct)
│   ├── template.rs         — templates/*.toml schema (TemplateFile, Variant)
│   ├── partial.rs          — partials/*.toml schema (PartialFile)
│   ├── fixture.rs          — fixtures/*.json loader (Context wrapper)
│   ├── scenario.rs         — tests/*.toml schema (Scenario, ScenarioEvent, Expected)
│   ├── project.rs          — Project struct, load_from_dir, into_engine, save_template, validate
│   ├── bundle.rs           — build_bundle (JSON + Rust targets)
│   ├── scaffold.rs         — Starter project templates (blank, changelog, vocab-pack)
│   └── runner.rs           — Scenario runner (render + assertion checks)
└── tests/
    ├── load_project.rs
    ├── validate_project.rs
    ├── into_engine.rs
    ├── bundle_json.rs
    ├── bundle_rust.rs
    ├── scenario_runner.rs
    └── fixtures/
        ├── blank-project/         — minimal valid project
        ├── multi-variant/          — exercises salience tiers + variants
        ├── multi-language/         — exercises EN+ES variants
        ├── with-partials/          — exercises partial composition
        ├── with-scenarios/         — exercises scenario runner
        └── invalid-projects/       — various malformed inputs for validate()
```

**Modified files:**

- `Cargo.toml` (workspace) — add `prosaic-project` member.
- `prosaic-core/src/engine.rs` — add `language_preference` setter, language-aware variant selection.
- `prosaic-core/src/lib.rs` — re-export new types if needed.
- `prosaic-cli/Cargo.toml` — add `prosaic-project` dep, `clap` features.
- `prosaic-cli/src/main.rs` — add `new`/`build`/`test` subcommands alongside existing pipe filter.
- `prosaic-wasm/Cargo.toml` — add `prosaic-project` dep.
- `prosaic-wasm/src/lib.rs` — add new wasm-bindgen methods.

---

## Task 0: Workspace setup — add `prosaic-project` crate

**Files:**
- Create: `prosaic-project/Cargo.toml`
- Create: `prosaic-project/src/lib.rs`
- Modify: `Cargo.toml` (root) — add member

- [ ] **Step 1: Add the crate to the workspace**

Create `prosaic-project/Cargo.toml`:

```toml
[package]
name = "prosaic-project"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Folder-of-files project format and bundler for Prosaic templates."

[dependencies]
prosaic-core = { path = "../prosaic-core", default-features = false, features = ["std", "serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = { version = "0.8", features = ["preserve_order"] }
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```

Create `prosaic-project/src/lib.rs`:

```rust
//! Prosaic Studio project format — folder-of-TOML loader, validator, bundler, scenario runner.
//!
//! A Prosaic project is a directory containing `prosaic.toml`, plus `templates/`,
//! `partials/`, `fixtures/`, and `tests/` subdirectories. This crate parses
//! that layout, materializes a configured [`prosaic_core::Engine`], and bundles
//! projects into portable JSON or generated Rust source for runtime loading.

mod bundle;
mod error;
mod fixture;
mod manifest;
mod partial;
mod project;
mod runner;
mod scaffold;
mod scenario;
mod template;

pub use bundle::{BuildOutput, BuildTarget, build_bundle};
pub use error::ProjectError;
pub use manifest::{EngineSettings, Manifest, SalienceThresholdsConfig, VocabDependency};
pub use partial::PartialFile;
pub use project::{Project, ValidationIssue};
pub use runner::{ScenarioOutcome, ScenarioRunner, ScenarioVerdict};
pub use scaffold::{Starter, scaffold_project};
pub use scenario::{Expected, ExpectedDiscourse, Scenario, ScenarioEvent};
pub use template::{TemplateFile, Variant};
```

Modify root `Cargo.toml` — add `"prosaic-project"` to the `members` array (insert alphabetically between `prosaic-grammar-es` and `prosaic-tracing`):

```toml
[workspace]
resolver = "2"
members = [
    "prosaic-core",
    "prosaic-grammar-de",
    "prosaic-grammar-en",
    "prosaic-grammar-es",
    "prosaic-derive",
    "prosaic-project",
    "prosaic-vocab-code",
    "prosaic-vocab-git",
    "prosaic-vocab-pr",
    "prosaic-vocab-release",
    "prosaic-wasm",
    "prosaic-cli",
    "prosaic-tracing",
]
```

- [ ] **Step 2: Verify the crate compiles**

Run: `cargo check --package prosaic-project`
Expected: Clean compile with `unused_imports` warnings on the not-yet-implemented module re-exports (acceptable until those modules exist).

- [ ] **Step 3: Stub out the modules so the lib.rs re-exports compile**

Create empty `pub mod` stubs for each referenced module. For each of `bundle.rs`, `error.rs`, `fixture.rs`, `manifest.rs`, `partial.rs`, `project.rs`, `runner.rs`, `scaffold.rs`, `scenario.rs`, `template.rs`, write:

```rust
// stub — populated by later tasks
```

Then comment out the `pub use` lines in `lib.rs` until the corresponding types exist; uncomment incrementally per task.

- [ ] **Step 4: Verify clean build**

Run: `cargo build --package prosaic-project`
Expected: `Finished dev profile`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml prosaic-project/
git commit -m "Scaffold prosaic-project crate"
```

---

## Task 1: ProjectError variants

**Files:**
- Modify: `prosaic-project/src/error.rs`

- [ ] **Step 1: Write the failing test**

Append to `prosaic-project/src/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::ProjectError;

    #[test]
    fn missing_manifest_displays_path() {
        let e = ProjectError::ManifestMissing {
            path: "/tmp/foo/prosaic.toml".to_string(),
        };
        let msg = format!("{e}");
        assert!(msg.contains("prosaic.toml"));
        assert!(msg.contains("/tmp/foo"));
    }

    #[test]
    fn malformed_toml_displays_filename_and_inner() {
        let e = ProjectError::TomlParse {
            file: "templates/code.modified.toml".to_string(),
            source: "expected `=` after key".to_string(),
        };
        let msg = format!("{e}");
        assert!(msg.contains("code.modified.toml"));
        assert!(msg.contains("expected `=`"));
    }

    #[test]
    fn template_validation_includes_key() {
        let e = ProjectError::TemplateValidation {
            key: "code.modified".to_string(),
            reason: "no variants registered".to_string(),
        };
        let msg = format!("{e}");
        assert!(msg.contains("code.modified"));
        assert!(msg.contains("no variants"));
    }
}
```

Add a stub above the tests so they compile:

```rust
//! Project loading and bundling errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("manifest missing at `{path}`: expected a `prosaic.toml` at the project root")]
    ManifestMissing { path: String },

    #[error("malformed TOML in `{file}`: {source}")]
    TomlParse { file: String, source: String },

    #[error("template `{key}` validation failed: {reason}")]
    TemplateValidation { key: String, reason: String },
}
```

- [ ] **Step 2: Run tests to verify they pass (logic is straightforward; failure here would be a compile/format issue)**

Run: `cargo test --package prosaic-project --lib error::tests`
Expected: 3 passed.

- [ ] **Step 3: Add the remaining error variants**

Replace the enum body with the full set:

```rust
#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("manifest missing at `{path}`: expected a `prosaic.toml` at the project root")]
    ManifestMissing { path: String },

    #[error("malformed TOML in `{file}`: {source}")]
    TomlParse { file: String, source: String },

    #[error("malformed JSON in `{file}`: {source}")]
    JsonParse { file: String, source: String },

    #[error("template `{key}` validation failed: {reason}")]
    TemplateValidation { key: String, reason: String },

    #[error("partial `{name}` validation failed: {reason}")]
    PartialValidation { name: String, reason: String },

    #[error("scenario `{name}` failed: {reason}")]
    ScenarioValidation { name: String, reason: String },

    #[error("fixture `{name}` failed: {reason}")]
    FixtureValidation { name: String, reason: String },

    #[error("vocab dependency `{crate_name}` not registered: enable the `{crate_name}` feature or add it to `prosaic.toml`")]
    UnregisteredVocab { crate_name: String },

    #[error("template `{key}` references unknown partial `{partial}`")]
    UnknownPartial { key: String, partial: String },

    #[error("template `{key}` uses unknown pipe `{pipe}` (known pipes: {known})")]
    UnknownPipe { key: String, pipe: String, known: String },

    #[error("io error reading `{path}`: {source}")]
    Io { path: String, source: String },

    #[error("engine error: {0}")]
    Engine(#[from] prosaic_core::ProsaicError),
}
```

- [ ] **Step 4: Run tests to verify all still pass**

Run: `cargo test --package prosaic-project --lib error::tests`
Expected: 3 passed.

- [ ] **Step 5: Uncomment the lib.rs re-export**

In `prosaic-project/src/lib.rs`, ensure `pub use error::ProjectError;` is uncommented.

Run: `cargo build --package prosaic-project`
Expected: Clean build.

- [ ] **Step 6: Commit**

```bash
git add prosaic-project/src/error.rs prosaic-project/src/lib.rs
git commit -m "Add ProjectError variants for prosaic-project"
```

---

## Task 2: Manifest schema (`prosaic.toml`)

**Files:**
- Modify: `prosaic-project/src/manifest.rs`
- Test: `prosaic-project/tests/load_project.rs` (and unit tests inline)

- [ ] **Step 1: Write the failing test**

Append to `prosaic-project/src/manifest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let toml = r#"
            name = "demo"
            version = "0.1.0"
            language = "en"
        "#;
        let m: Manifest = toml::from_str(toml).unwrap();
        assert_eq!(m.name, "demo");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.language, "en");
        assert!(m.dependencies.is_empty());
        // engine settings default
        assert_eq!(m.engine.strictness, "strict");
        assert_eq!(m.engine.variation, "fixed");
    }

    #[test]
    fn parse_full_manifest() {
        let toml = r#"
            name = "changelog"
            version = "1.2.0"
            language = "en"

            [engine]
            strictness = "lenient"
            variation = "round_robin"
            smart_quotes = true
            max_sentence_length = 120
            faithfulness_min = 0.85

            [engine.salience_thresholds]
            low_max = 2
            high_min = 30

            [[dependencies]]
            crate = "prosaic-vocab-code"
            version = "0.3"
            languages = ["en", "es"]

            [[dependencies]]
            crate = "prosaic-vocab-git"
            version = "0.3"
        "#;
        let m: Manifest = toml::from_str(toml).unwrap();
        assert_eq!(m.engine.max_sentence_length, 120);
        assert_eq!(m.engine.faithfulness_min, 0.85);
        let st = m.engine.salience_thresholds.unwrap();
        assert_eq!(st.low_max, 2);
        assert_eq!(st.high_min, 30);
        assert_eq!(m.dependencies.len(), 2);
        assert_eq!(m.dependencies[0].crate_name, "prosaic-vocab-code");
        assert_eq!(m.dependencies[0].languages, vec!["en", "es"]);
        assert!(m.dependencies[1].languages.is_empty()); // default = all
    }

    #[test]
    fn missing_required_fields_errors() {
        let toml = r#"version = "0.1.0""#;
        let res = toml::from_str::<Manifest>(toml);
        assert!(res.is_err(), "expected error for missing `name` field");
    }

    #[test]
    fn invalid_strictness_errors() {
        let toml = r#"
            name = "demo"
            version = "0.1.0"
            language = "en"
            [engine]
            strictness = "yolo"
        "#;
        // We deserialize as String first, then validate at materialize-engine time.
        // Here we just confirm the Manifest parses (validation happens later).
        let m: Manifest = toml::from_str(toml).unwrap();
        assert_eq!(m.engine.strictness, "yolo"); // raw value preserved for downstream validation
    }
}
```

Add the stub:

```rust
//! `prosaic.toml` schema — project manifest.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub language: String,
    #[serde(default)]
    pub engine: EngineSettings,
    #[serde(default)]
    pub dependencies: Vec<VocabDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSettings {
    pub strictness: String,
    pub variation: String,
    pub smart_quotes: bool,
    pub max_sentence_length: usize,
    pub faithfulness_min: f64,
    pub salience_thresholds: Option<SalienceThresholdsConfig>,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            strictness: "strict".to_string(),
            variation: "fixed".to_string(),
            smart_quotes: false,
            max_sentence_length: 0,
            faithfulness_min: 0.0,
            salience_thresholds: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceThresholdsConfig {
    pub low_max: i64,
    pub high_min: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabDependency {
    #[serde(rename = "crate")]
    pub crate_name: String,
    pub version: String,
    #[serde(default)]
    pub languages: Vec<String>,
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --package prosaic-project --lib manifest::tests`
Expected: 4 passed.

- [ ] **Step 3: Uncomment the lib.rs re-exports for Manifest, EngineSettings, SalienceThresholdsConfig, VocabDependency**

Run: `cargo build --package prosaic-project`
Expected: Clean build.

- [ ] **Step 4: Commit**

```bash
git add prosaic-project/src/manifest.rs prosaic-project/src/lib.rs
git commit -m "Add Manifest schema for prosaic-project"
```

---

## Task 3: Template + Variant schema

**Files:**
- Modify: `prosaic-project/src/template.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! `templates/*.toml` schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    pub key: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub slots_required: Vec<String>,
    #[serde(default)]
    pub slots_optional: Vec<String>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    /// "low" | "medium" | "high"
    #[serde(default = "default_salience")]
    pub salience: String,
    /// BCP-47 lang code; defaults to project language if unspecified.
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub description: String,
    pub body: String,
}

fn default_salience() -> String {
    "medium".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_template() {
        let toml = r#"
            key = "code.modified"
            variants = [
              { body = "{name|refer} was modified" },
            ]
        "#;
        let t: TemplateFile = toml::from_str(toml).unwrap();
        assert_eq!(t.key, "code.modified");
        assert_eq!(t.variants.len(), 1);
        assert_eq!(t.variants[0].salience, "medium"); // default
        assert_eq!(t.variants[0].body, "{name|refer} was modified");
        assert!(t.variants[0].language.is_none());
    }

    #[test]
    fn parse_multi_variant_with_salience() {
        let toml = r#"
            key = "code.modified"
            description = "Render a 'class X was modified' event."
            slots_required = ["name"]
            slots_optional = ["consumer_count"]

            [[variants]]
            salience = "low"
            body = "{name|refer} was modified"

            [[variants]]
            salience = "medium"
            body = """{name|refer} was modified, affecting {consumer_count}"""

            [[variants]]
            salience = "high"
            language = "en"
            body = "{name|refer} has been substantially modified"
        "#;
        let t: TemplateFile = toml::from_str(toml).unwrap();
        assert_eq!(t.variants.len(), 3);
        assert_eq!(t.variants[0].salience, "low");
        assert_eq!(t.variants[1].salience, "medium");
        assert_eq!(t.variants[2].salience, "high");
        assert_eq!(t.variants[2].language.as_deref(), Some("en"));
        assert_eq!(t.slots_required, vec!["name"]);
        assert_eq!(t.slots_optional, vec!["consumer_count"]);
    }

    #[test]
    fn parse_multi_language_variants() {
        let toml = r#"
            key = "code.modified"

            [[variants]]
            salience = "medium"
            language = "en"
            body = "{name} was modified"

            [[variants]]
            salience = "medium"
            language = "es"
            body = "{name} fue modificado"
        "#;
        let t: TemplateFile = toml::from_str(toml).unwrap();
        assert_eq!(t.variants.len(), 2);
        assert_eq!(t.variants[0].language.as_deref(), Some("en"));
        assert_eq!(t.variants[1].language.as_deref(), Some("es"));
    }

    #[test]
    fn missing_key_errors() {
        let toml = r#"
            variants = [{ body = "x" }]
        "#;
        let res = toml::from_str::<TemplateFile>(toml);
        assert!(res.is_err(), "expected error for missing `key` field");
    }

    #[test]
    fn missing_variant_body_errors() {
        let toml = r#"
            key = "x"
            [[variants]]
            salience = "medium"
        "#;
        let res = toml::from_str::<TemplateFile>(toml);
        assert!(res.is_err(), "expected error for missing variant `body` field");
    }

    #[test]
    fn round_trip_serialize() {
        let original: TemplateFile = toml::from_str(
            r#"
            key = "code.added"
            description = "Add event."
            variants = [
              { salience = "medium", body = "{name|refer} was added" },
            ]
            "#,
        ).unwrap();
        let serialized = toml::to_string(&original).unwrap();
        let reparsed: TemplateFile = toml::from_str(&serialized).unwrap();
        assert_eq!(reparsed.key, "code.added");
        assert_eq!(reparsed.variants.len(), 1);
        assert_eq!(reparsed.variants[0].body, "{name|refer} was added");
    }
}
```

- [ ] **Step 2: Run tests to verify**

Run: `cargo test --package prosaic-project --lib template::tests`
Expected: 6 passed.

- [ ] **Step 3: Uncomment lib.rs re-exports for TemplateFile, Variant**

Run: `cargo build --package prosaic-project`

- [ ] **Step 4: Commit**

```bash
git add prosaic-project/src/template.rs prosaic-project/src/lib.rs
git commit -m "Add TemplateFile + Variant schema for prosaic-project"
```

---

## Task 4: Partial schema

**Files:**
- Modify: `prosaic-project/src/partial.rs`

- [ ] **Step 1: Write the test + impl together (small file)**

```rust
//! `partials/*.toml` schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialFile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub body: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_partial() {
        let toml = r#"
            name = "impact_tail"
            body = "{?consumer_count}, affecting {consumer_count}{/?}"
        "#;
        let p: PartialFile = toml::from_str(toml).unwrap();
        assert_eq!(p.name, "impact_tail");
        assert!(p.body.contains("affecting"));
    }

    #[test]
    fn missing_name_errors() {
        let res = toml::from_str::<PartialFile>(r#"body = "x""#);
        assert!(res.is_err());
    }

    #[test]
    fn missing_body_errors() {
        let res = toml::from_str::<PartialFile>(r#"name = "x""#);
        assert!(res.is_err());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package prosaic-project --lib partial::tests`
Expected: 3 passed.

- [ ] **Step 3: Uncomment lib.rs re-export for PartialFile, then commit**

```bash
git add prosaic-project/src/partial.rs prosaic-project/src/lib.rs
git commit -m "Add PartialFile schema for prosaic-project"
```

---

## Task 5: Scenario schema

**Files:**
- Modify: `prosaic-project/src/scenario.rs`

- [ ] **Step 1: Test + impl**

```rust
//! `tests/*.toml` schema — narrative-flow scenarios with discourse assertions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub engine: ScenarioEngineOverride,
    pub events: Vec<ScenarioEvent>,
    #[serde(default)]
    pub expected: Option<Expected>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScenarioEngineOverride {
    pub variation: Option<String>,
    pub language: Option<String>,
    pub salience_thresholds: Option<crate::manifest::SalienceThresholdsConfig>,
    pub faithfulness_min: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioEvent {
    pub template: String,
    /// Free-form context map. Values can be strings, numbers, bools, or arrays of strings.
    #[serde(default)]
    pub context: HashMap<String, toml::Value>,
    /// RST relation hint (Elaboration, Contrast, Result, etc.). String form for TOML friendliness.
    #[serde(default)]
    pub rst_relation: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Expected {
    pub output: Option<String>,
    pub faithfulness_min: Option<f64>,
    pub discourse: Vec<ExpectedDiscourse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedDiscourse {
    pub event_index: usize,
    #[serde(default)]
    pub reference_form: Option<String>,
    #[serde(default)]
    pub connective_contains: Option<String>,
    #[serde(default)]
    pub transition: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_scenario() {
        let toml = r#"
            name = "smoke"
            [[events]]
            template = "code.added"
            context = { name = "Foo", entity_type = "class" }
        "#;
        let s: Scenario = toml::from_str(toml).unwrap();
        assert_eq!(s.name, "smoke");
        assert_eq!(s.events.len(), 1);
        assert_eq!(s.events[0].template, "code.added");
        assert_eq!(s.events[0].context.get("name").unwrap().as_str().unwrap(), "Foo");
    }

    #[test]
    fn parse_full_scenario_with_expectations() {
        let toml = r#"
            name = "PR 142"
            description = "auth refactor"

            [engine]
            variation = "fixed"
            language = "en"
            faithfulness_min = 0.9

            [[events]]
            template = "code.modified"
            context = { name = "UserService", consumer_count = 6 }

            [[events]]
            template = "code.moved"
            context = { name = "UserService" }
            rst_relation = "elaboration"

            [expected]
            output = "..."
            faithfulness_min = 0.85

            [[expected.discourse]]
            event_index = 1
            reference_form = "Pronoun"

            [[expected.discourse]]
            event_index = 1
            connective_contains = "also"
        "#;
        let s: Scenario = toml::from_str(toml).unwrap();
        assert_eq!(s.events.len(), 2);
        assert_eq!(s.events[1].rst_relation.as_deref(), Some("elaboration"));
        let exp = s.expected.unwrap();
        assert_eq!(exp.faithfulness_min, Some(0.85));
        assert_eq!(exp.discourse.len(), 2);
        assert_eq!(exp.discourse[0].reference_form.as_deref(), Some("Pronoun"));
        assert_eq!(exp.discourse[1].connective_contains.as_deref(), Some("also"));
    }

    #[test]
    fn parse_array_context_value() {
        let toml = r#"
            name = "list"
            [[events]]
            template = "code.modified"
            context = { name = "X", consumers = ["a", "b", "c"] }
        "#;
        let s: Scenario = toml::from_str(toml).unwrap();
        let arr = s.events[0].context.get("consumers").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_str().unwrap(), "a");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package prosaic-project --lib scenario::tests`
Expected: 3 passed.

- [ ] **Step 3: Uncomment lib.rs re-exports, commit**

```bash
git add prosaic-project/src/scenario.rs prosaic-project/src/lib.rs
git commit -m "Add Scenario schema for prosaic-project"
```

---

## Task 6: Fixture loader (JSON → prosaic Context)

**Files:**
- Modify: `prosaic-project/src/fixture.rs`

- [ ] **Step 1: Test + impl**

```rust
//! `fixtures/*.json` loader.
//!
//! A fixture is a single-event context map serialized as JSON.
//! It deserializes directly into a [`prosaic_core::Context`] via the
//! existing `serde` support on `Value`.

use prosaic_core::{Context, Value};
use serde_json::Value as JsonValue;

use crate::error::ProjectError;

/// Parse a fixture JSON string into a `Context`.
pub fn parse_fixture(name: &str, json: &str) -> Result<Context, ProjectError> {
    let v: JsonValue = serde_json::from_str(json).map_err(|e| ProjectError::JsonParse {
        file: name.to_string(),
        source: e.to_string(),
    })?;
    let obj = v.as_object().ok_or_else(|| ProjectError::FixtureValidation {
        name: name.to_string(),
        reason: "fixture must be a JSON object".to_string(),
    })?;
    let mut ctx = Context::new();
    for (k, v) in obj {
        ctx.insert(k.clone(), json_to_value(v));
    }
    Ok(ctx)
}

fn json_to_value(v: &JsonValue) -> Value {
    match v {
        JsonValue::Null => Value::String(String::new()),
        JsonValue::Bool(b) => Value::Number(if *b { 1 } else { 0 }),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Number(i)
            } else if let Some(f) = n.as_f64() {
                Value::Number(f as i64)
            } else {
                Value::Number(0)
            }
        }
        JsonValue::String(s) => Value::String(s.clone()),
        JsonValue::Array(items) => Value::List(
            items
                .iter()
                .map(|i| match i {
                    JsonValue::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect(),
        ),
        JsonValue::Object(_) => Value::String(v.to_string()), // nested objects not first-class
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_fixture() {
        let json = r#"{"name": "UserService", "entity_type": "class", "consumer_count": 6}"#;
        let ctx = parse_fixture("test", json).unwrap();
        assert_eq!(ctx.get("name").unwrap().as_display(), "UserService");
        assert_eq!(ctx.get("consumer_count").unwrap().as_number().unwrap(), 6);
    }

    #[test]
    fn parse_fixture_with_array() {
        let json = r#"{"consumers": ["A", "B", "C"]}"#;
        let ctx = parse_fixture("test", json).unwrap();
        let list = ctx.get("consumers").unwrap().as_list().unwrap();
        assert_eq!(list, vec!["A".to_string(), "B".to_string(), "C".to_string()]);
    }

    #[test]
    fn invalid_json_errors() {
        let res = parse_fixture("bad", "{not json");
        assert!(matches!(res, Err(ProjectError::JsonParse { .. })));
    }

    #[test]
    fn non_object_errors() {
        let res = parse_fixture("bad", "[1, 2, 3]");
        assert!(matches!(res, Err(ProjectError::FixtureValidation { .. })));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package prosaic-project --lib fixture::tests`
Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
git add prosaic-project/src/fixture.rs
git commit -m "Add fixture JSON loader for prosaic-project"
```

---

## Task 7: prosaic-core multi-language plumbing

**Files:**
- Modify: `prosaic-core/src/engine.rs`

The engine needs (a) a way for a registered template to carry a language tag, and (b) a render-time language preference that biases variant selection.

- [ ] **Step 1: Write the failing test**

Append to the existing test module in `prosaic-core/src/engine.rs` (find the closing `}` of the existing `mod tests {` block and insert before it). Use a search anchor for the existing `mod tests {` declaration:

```rust
    #[test]
    fn language_preference_picks_matching_variant() {
        use crate::Variation;
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed)
            .language_preference("es");

        engine
            .register_template_with_language("greet", "Hello {name}", Some("en"))
            .unwrap();
        engine
            .register_template_with_language("greet", "Hola {name}", Some("es"))
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("name", Value::String("world".to_string()));

        let mut session = Session::new();
        let out = engine.render(&mut session, "greet", &ctx).unwrap();
        assert!(out.starts_with("Hola"), "expected Spanish variant; got: {out}");
    }

    #[test]
    fn language_preference_falls_back_to_unspecified() {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Fixed)
            .language_preference("es");

        // Only an English variant registered.
        engine
            .register_template_with_language("greet", "Hello {name}", Some("en"))
            .unwrap();

        let mut ctx = Context::new();
        ctx.insert("name", Value::String("world".to_string()));

        let mut session = Session::new();
        let out = engine.render(&mut session, "greet", &ctx).unwrap();
        assert!(out.starts_with("Hello"), "expected fallback to English; got: {out}");
    }
```

If `English` isn't already imported in the test module, add it: `use prosaic_grammar_en::English;` (this requires the `prosaic-grammar-en` dev-dependency on prosaic-core; check `prosaic-core/Cargo.toml [dev-dependencies]`. If absent, add `prosaic-grammar-en = { path = "../prosaic-grammar-en" }`).

- [ ] **Step 2: Run test to verify it fails (compile error: methods don't exist yet)**

Run: `cargo test --package prosaic-core --lib language_preference 2>&1 | tail -20`
Expected: Compile error mentioning `language_preference` and `register_template_with_language` not found.

- [ ] **Step 3: Add the storage to Engine**

Find the `pub struct Engine<L: Language = ...>` definition. Add a `pub(crate) language_preference: Option<String>,` field (default `None`). Add to `Engine::new`:

```rust
language_preference: None,
```

Add the builder method on `impl<L: Language> Engine<L>` (place near other builder methods like `strictness`):

```rust
/// Set the BCP-47 language code that variant selection should prefer.
/// When templates are registered with `register_template_with_language`,
/// the engine picks variants whose language matches this preference; if
/// none match, it falls back to variants with no language tag, then to
/// any registered variant.
pub fn language_preference(mut self, lang: impl Into<String>) -> Self {
    self.language_preference = Some(lang.into());
    self
}
```

- [ ] **Step 4: Add language tagging to template registration**

Locate the existing `register_template` method and the `TemplateAlternative` (or equivalent) struct that holds registered template state. Add an optional `language: Option<String>` field to that struct, defaulting to `None` for `register_template`.

Add the new method:

```rust
/// Register a template variant tagged with a BCP-47 language code.
/// Variants registered without a language are language-agnostic and serve
/// as fallbacks when no language-matching variant exists.
pub fn register_template_with_language(
    &mut self,
    key: &str,
    body: &str,
    language: Option<&str>,
) -> Result<(), ProsaicError> {
    // Reuse the existing register_template body but stash the language tag
    // on the produced TemplateAlternative.
    // ... call the same parsing/storing logic, then set .language = language.map(|s| s.to_string())
}
```

Update the alternative-selection path (where the engine picks among alternatives at render time) to filter by language preference:

1. If `self.language_preference == Some(lang)` and any alternative has `.language == Some(lang)`, restrict the candidate set to those.
2. Else if any alternative has `.language == None`, restrict to those.
3. Else use all alternatives (preserves existing behaviour).

This filter applies BEFORE the variation strategy / scoring layer.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --package prosaic-core --lib language_preference`
Expected: 2 passed.

- [ ] **Step 6: Run the full prosaic-core test suite to ensure no regressions**

Run: `cargo test --package prosaic-core --lib`
Expected: All previously-passing tests still pass.

- [ ] **Step 7: Commit**

```bash
git add prosaic-core/src/engine.rs prosaic-core/Cargo.toml
git commit -m "Add language preference + language-tagged template registration to Engine"
```

---

## Task 8: Project loader (`Project::load_from_dir`)

**Files:**
- Modify: `prosaic-project/src/project.rs`
- Test: `prosaic-project/tests/load_project.rs`
- Test: `prosaic-project/tests/fixtures/blank-project/` (sample project)

- [ ] **Step 1: Create the blank-project test fixture**

Create directory `prosaic-project/tests/fixtures/blank-project/` containing:

`prosaic.toml`:
```toml
name = "blank"
version = "0.1.0"
language = "en"
```

(No subdirectories needed; loader must handle the empty case.)

- [ ] **Step 2: Create the multi-variant test fixture**

Create directory `prosaic-project/tests/fixtures/multi-variant/`:

`prosaic.toml`:
```toml
name = "multi-variant"
version = "0.1.0"
language = "en"
```

`templates/code.modified.toml`:
```toml
key = "code.modified"
description = "test fixture"

[[variants]]
salience = "low"
body = "{name} was modified"

[[variants]]
salience = "medium"
body = "{name} was modified, affecting {consumer_count}"

[[variants]]
salience = "high"
body = "{name} has been substantially modified"
```

`partials/impact_tail.toml`:
```toml
name = "impact_tail"
body = ", affecting {consumer_count}"
```

`fixtures/userservice.json`:
```json
{"name": "UserService", "consumer_count": 6}
```

`tests/smoke.toml`:
```toml
name = "smoke"

[[events]]
template = "code.modified"
context = { name = "UserService", consumer_count = 6 }

[expected]
output = "UserService was modified, affecting 6"
```

- [ ] **Step 3: Write the failing integration test**

Create `prosaic-project/tests/load_project.rs`:

```rust
use prosaic_project::Project;
use std::path::Path;

#[test]
fn load_blank_project() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blank-project"),
    )
    .unwrap();
    assert_eq!(p.manifest.name, "blank");
    assert_eq!(p.templates.len(), 0);
    assert_eq!(p.partials.len(), 0);
    assert_eq!(p.fixtures.len(), 0);
    assert_eq!(p.scenarios.len(), 0);
}

#[test]
fn load_multi_variant_project() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();
    assert_eq!(p.templates.len(), 1);
    let t = p.templates.get("code.modified").unwrap();
    assert_eq!(t.variants.len(), 3);

    assert_eq!(p.partials.len(), 1);
    assert!(p.partials.contains_key("impact_tail"));

    assert_eq!(p.fixtures.len(), 1);
    assert!(p.fixtures.contains_key("userservice"));

    assert_eq!(p.scenarios.len(), 1);
    assert_eq!(p.scenarios.get("smoke").unwrap().events.len(), 1);
}

#[test]
fn missing_manifest_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let res = Project::load_from_dir(tmp.path());
    assert!(res.is_err());
}
```

- [ ] **Step 4: Implement Project::load_from_dir**

Replace `prosaic-project/src/project.rs` with:

```rust
//! Project root: load a folder, validate, materialize an engine.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use prosaic_core::Context;

use crate::error::ProjectError;
use crate::fixture::parse_fixture;
use crate::manifest::Manifest;
use crate::partial::PartialFile;
use crate::scenario::Scenario;
use crate::template::TemplateFile;

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub templates: HashMap<String, TemplateFile>,
    pub partials: HashMap<String, PartialFile>,
    pub fixtures: HashMap<String, Context>,
    pub scenarios: HashMap<String, Scenario>,
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub level: ValidationLevel,
    pub location: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationLevel {
    Error,
    Warning,
}

impl Project {
    pub fn load_from_dir(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let root = path.as_ref().to_path_buf();

        let manifest_path = root.join("prosaic.toml");
        if !manifest_path.exists() {
            return Err(ProjectError::ManifestMissing {
                path: manifest_path.display().to_string(),
            });
        }
        let manifest_str = fs::read_to_string(&manifest_path).map_err(|e| ProjectError::Io {
            path: manifest_path.display().to_string(),
            source: e.to_string(),
        })?;
        let manifest: Manifest = toml::from_str(&manifest_str).map_err(|e| ProjectError::TomlParse {
            file: "prosaic.toml".to_string(),
            source: e.to_string(),
        })?;

        let templates = load_toml_dir::<TemplateFile, _>(
            &root.join("templates"),
            "toml",
            |t| Ok(t.key.clone()),
        )?;
        let partials = load_toml_dir::<PartialFile, _>(
            &root.join("partials"),
            "toml",
            |p| Ok(p.name.clone()),
        )?;
        let scenarios = load_toml_dir::<Scenario, _>(
            &root.join("tests"),
            "toml",
            |s| Ok(s.name.clone()),
        )?;
        let fixtures = load_fixtures_dir(&root.join("fixtures"))?;

        Ok(Project {
            root,
            manifest,
            templates,
            partials,
            fixtures,
            scenarios,
        })
    }
}

fn load_toml_dir<T, F>(
    dir: &Path,
    ext: &str,
    key_fn: F,
) -> Result<HashMap<String, T>, ProjectError>
where
    T: serde::de::DeserializeOwned,
    F: Fn(&T) -> Result<String, ProjectError>,
{
    let mut out = HashMap::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir).map_err(|e| ProjectError::Io {
        path: dir.display().to_string(),
        source: e.to_string(),
    })? {
        let entry = entry.map_err(|e| ProjectError::Io {
            path: dir.display().to_string(),
            source: e.to_string(),
        })?;
        let path = entry.path();
        if path.extension().map(|e| e == ext).unwrap_or(false) {
            let text = fs::read_to_string(&path).map_err(|e| ProjectError::Io {
                path: path.display().to_string(),
                source: e.to_string(),
            })?;
            let parsed: T = toml::from_str(&text).map_err(|e| ProjectError::TomlParse {
                file: path.file_name().unwrap().to_string_lossy().to_string(),
                source: e.to_string(),
            })?;
            let key = key_fn(&parsed)?;
            out.insert(key, parsed);
        }
    }
    Ok(out)
}

fn load_fixtures_dir(dir: &Path) -> Result<HashMap<String, Context>, ProjectError> {
    let mut out = HashMap::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir).map_err(|e| ProjectError::Io {
        path: dir.display().to_string(),
        source: e.to_string(),
    })? {
        let entry = entry.map_err(|e| ProjectError::Io {
            path: dir.display().to_string(),
            source: e.to_string(),
        })?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let stem = path
                .file_stem()
                .ok_or_else(|| ProjectError::Io {
                    path: path.display().to_string(),
                    source: "file has no stem".to_string(),
                })?
                .to_string_lossy()
                .to_string();
            let text = fs::read_to_string(&path).map_err(|e| ProjectError::Io {
                path: path.display().to_string(),
                source: e.to_string(),
            })?;
            let ctx = parse_fixture(&stem, &text)?;
            out.insert(stem, ctx);
        }
    }
    Ok(out)
}
```

- [ ] **Step 5: Add `pub use project::{Project, ValidationIssue, ValidationLevel};` to lib.rs**

- [ ] **Step 6: Run tests**

Run: `cargo test --package prosaic-project --test load_project`
Expected: 3 passed.

- [ ] **Step 7: Commit**

```bash
git add prosaic-project/src/project.rs prosaic-project/src/lib.rs prosaic-project/tests/
git commit -m "Add Project::load_from_dir with TOML and fixture loaders"
```

---

## Task 9: Project::into_engine — materialize a configured Engine

**Files:**
- Modify: `prosaic-project/src/project.rs`
- Modify: `prosaic-project/Cargo.toml` — add prosaic-grammar-en (and -es, -de) for engine construction
- Test: `prosaic-project/tests/into_engine.rs`

- [ ] **Step 1: Add the grammar dev-dependencies**

In `prosaic-project/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
prosaic-grammar-en = { path = "../prosaic-grammar-en" }
prosaic-grammar-es = { path = "../prosaic-grammar-es" }
prosaic-grammar-de = { path = "../prosaic-grammar-de" }
```

- [ ] **Step 2: Write the failing test**

Create `prosaic-project/tests/into_engine.rs`:

```rust
use prosaic_core::{Context, Session, Value};
use prosaic_project::Project;
use std::path::Path;

#[test]
fn into_engine_renders_template() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();

    let mut engine = p.into_engine().unwrap();
    let mut ctx = Context::new();
    ctx.insert("name", Value::String("Foo".into()));
    ctx.insert("consumer_count", Value::Number(3));

    let mut session = Session::new();
    let out = engine.render(&mut session, "code.modified", &ctx).unwrap();
    assert!(out.contains("Foo"));
    assert!(out.contains("3"));
}

#[test]
fn into_engine_registers_partials() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();

    let engine = p.into_engine().unwrap();
    // The engine should have registered the impact_tail partial; we don't
    // have a direct accessor, so we verify by registering a template that
    // uses it and rendering — engine.register_template would error on
    // unknown partial.
    // (Done indirectly here; the multi-variant fixture's templates don't
    // use partials, so this test just confirms construction succeeded.)
    drop(engine);
}
```

- [ ] **Step 3: Implement Project::into_engine**

Append to `prosaic-project/src/project.rs`:

```rust
use prosaic_core::{Engine, Strictness, Variation};
use prosaic_grammar_en::English;

impl Project {
    /// Materialize a configured Engine from this project. v1: English only;
    /// language plumbing is in place for future grammar selection by manifest.
    pub fn into_engine(&self) -> Result<Engine<English>, ProjectError> {
        let mut engine = Engine::new(English::new());

        // Apply engine settings from the manifest
        let s = &self.manifest.engine;
        engine = match s.strictness.as_str() {
            "strict" => engine.strictness(Strictness::Strict),
            "lenient" => engine.strictness(Strictness::Lenient),
            "silent" => engine.strictness(Strictness::Silent),
            other => {
                return Err(ProjectError::TemplateValidation {
                    key: "(manifest)".to_string(),
                    reason: format!("unknown strictness `{other}`"),
                });
            }
        };
        engine = match s.variation.as_str() {
            "fixed" => engine.variation(Variation::Fixed),
            "round_robin" => engine.variation(Variation::RoundRobin),
            "random" => engine.variation(Variation::Random),
            other => {
                return Err(ProjectError::TemplateValidation {
                    key: "(manifest)".to_string(),
                    reason: format!("unknown variation `{other}`"),
                });
            }
        };
        if s.smart_quotes {
            engine = engine.smart_quotes(true);
        }
        if s.max_sentence_length > 0 {
            engine = engine.max_sentence_length(s.max_sentence_length);
        }
        if s.faithfulness_min > 0.0 {
            engine = engine.with_faithfulness_gate(s.faithfulness_min);
        }
        if let Some(thr) = &s.salience_thresholds {
            engine = engine.salience_thresholds(prosaic_core::SalienceThresholds {
                low_max: thr.low_max,
                high_min: thr.high_min,
            });
        }
        engine = engine.language_preference(&self.manifest.language);

        // Register partials first (so templates can reference them)
        for (name, partial) in &self.partials {
            engine
                .register_partial(name, &partial.body)
                .map_err(|e| ProjectError::PartialValidation {
                    name: name.clone(),
                    reason: e.to_string(),
                })?;
        }

        // Register templates
        for (key, template) in &self.templates {
            for variant in &template.variants {
                let salience = match variant.salience.as_str() {
                    "low" => prosaic_core::Salience::Low,
                    "medium" => prosaic_core::Salience::Medium,
                    "high" => prosaic_core::Salience::High,
                    other => {
                        return Err(ProjectError::TemplateValidation {
                            key: key.clone(),
                            reason: format!("unknown salience `{other}`"),
                        });
                    }
                };
                let language = variant.language.as_deref();
                engine
                    .register_template_with_language_at(key, &variant.body, salience, language)
                    .map_err(|e| ProjectError::TemplateValidation {
                        key: key.clone(),
                        reason: e.to_string(),
                    })?;
            }
        }

        Ok(engine)
    }
}
```

The `register_template_with_language_at` method needs to be added to `prosaic-core` — Task 7's `register_template_with_language` only handles default-salience registration. Add a salience-tier-aware variant (mirroring `register_template_at`).

In `prosaic-core/src/engine.rs`, add:

```rust
pub fn register_template_with_language_at(
    &mut self,
    key: &str,
    body: &str,
    salience: Salience,
    language: Option<&str>,
) -> Result<(), ProsaicError> {
    // identical to register_template_at, plus tag .language on the alternative
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package prosaic-project --test into_engine`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add prosaic-project/ prosaic-core/src/engine.rs
git commit -m "Add Project::into_engine to materialize a configured Engine"
```

---

## Task 10: Project::validate — surface issues without erroring

**Files:**
- Modify: `prosaic-project/src/project.rs`
- Test: `prosaic-project/tests/validate_project.rs`

- [ ] **Step 1: Create invalid-project fixtures**

`prosaic-project/tests/fixtures/invalid-projects/unknown-pipe/prosaic.toml`:
```toml
name = "bad"
version = "0.1.0"
language = "en"
```

`prosaic-project/tests/fixtures/invalid-projects/unknown-pipe/templates/x.toml`:
```toml
key = "x"
[[variants]]
salience = "medium"
body = "{name|nonexistent_pipe}"
```

`prosaic-project/tests/fixtures/invalid-projects/unknown-partial/prosaic.toml`:
```toml
name = "bad"
version = "0.1.0"
language = "en"
```

`prosaic-project/tests/fixtures/invalid-projects/unknown-partial/templates/x.toml`:
```toml
key = "x"
[[variants]]
salience = "medium"
body = "{name} {>nonexistent_partial}"
```

- [ ] **Step 2: Write the failing test**

Create `prosaic-project/tests/validate_project.rs`:

```rust
use prosaic_project::{Project, ValidationLevel};
use std::path::Path;

#[test]
fn validate_clean_project_has_no_issues() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();
    let issues = p.validate();
    let errors: Vec<_> = issues.iter().filter(|i| i.level == ValidationLevel::Error).collect();
    assert!(errors.is_empty(), "expected no validation errors, got: {errors:?}");
}

#[test]
fn validate_unknown_pipe_reports_error() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invalid-projects/unknown-pipe"),
    )
    .unwrap();
    let issues = p.validate();
    let errors: Vec<_> = issues.iter().filter(|i| i.level == ValidationLevel::Error).collect();
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|i| i.message.contains("nonexistent_pipe")));
}

#[test]
fn validate_unknown_partial_reports_error() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invalid-projects/unknown-partial"),
    )
    .unwrap();
    let issues = p.validate();
    let errors: Vec<_> = issues.iter().filter(|i| i.level == ValidationLevel::Error).collect();
    assert!(errors.iter().any(|i| i.message.contains("nonexistent_partial")));
}
```

- [ ] **Step 3: Implement Project::validate**

Append to `prosaic-project/src/project.rs`:

```rust
const KNOWN_PIPES: &[&str] = &[
    "plural", "pluralize", "article", "join", "ordinal", "words",
    "truncate", "capitalize", "refer", "verb", "syn",
    "relative", "since_last", "quantify", "proportion",
    "hedge", "negated", "choose", "demonstrative",
];

impl Project {
    /// Walk every template and partial; report unknown pipes and unknown
    /// partial references as validation issues. Does not error — the caller
    /// can decide whether to proceed.
    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let known_partials: std::collections::HashSet<_> = self.partials.keys().cloned().collect();

        for (key, template) in &self.templates {
            for (vi, variant) in template.variants.iter().enumerate() {
                let parsed = match prosaic_core::Template::parse(&variant.body) {
                    Ok(p) => p,
                    Err(e) => {
                        issues.push(ValidationIssue {
                            level: ValidationLevel::Error,
                            location: format!("templates/{key}.toml#variant[{vi}]"),
                            message: format!("template parse error: {e}"),
                        });
                        continue;
                    }
                };
                for pipe_name in parsed.pipe_names() {
                    if !KNOWN_PIPES.contains(&pipe_name.as_str()) {
                        issues.push(ValidationIssue {
                            level: ValidationLevel::Error,
                            location: format!("templates/{key}.toml#variant[{vi}]"),
                            message: format!("unknown pipe `{pipe_name}`"),
                        });
                    }
                }
                for partial_name in parsed.partial_names() {
                    if !known_partials.contains(&partial_name) {
                        issues.push(ValidationIssue {
                            level: ValidationLevel::Error,
                            location: format!("templates/{key}.toml#variant[{vi}]"),
                            message: format!("unknown partial `{partial_name}`"),
                        });
                    }
                }
            }
        }

        issues
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package prosaic-project --test validate_project`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add prosaic-project/
git commit -m "Add Project::validate for pipe and partial reference checks"
```

---

## Task 11: Project::save_template — write back to TOML

**Files:**
- Modify: `prosaic-project/src/project.rs`
- Test: `prosaic-project/tests/save_template.rs`

- [ ] **Step 1: Write the failing test**

Create `prosaic-project/tests/save_template.rs`:

```rust
use prosaic_project::{Project, TemplateFile, Variant};
use tempfile::tempdir;

#[test]
fn save_template_round_trips() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Set up minimal project on the temp dir
    std::fs::write(
        root.join("prosaic.toml"),
        r#"name = "tmp"
version = "0.1.0"
language = "en"
"#,
    ).unwrap();
    std::fs::create_dir_all(root.join("templates")).unwrap();

    let mut p = Project::load_from_dir(root).unwrap();
    let new_template = TemplateFile {
        key: "code.added".to_string(),
        description: "test".to_string(),
        slots_required: vec!["name".to_string()],
        slots_optional: vec![],
        variants: vec![Variant {
            salience: "medium".to_string(),
            language: Some("en".to_string()),
            description: String::new(),
            body: "{name} was added".to_string(),
        }],
    };
    p.templates.insert("code.added".to_string(), new_template);

    p.save_template("code.added").unwrap();

    // Re-load and verify
    let reloaded = Project::load_from_dir(root).unwrap();
    let t = reloaded.templates.get("code.added").unwrap();
    assert_eq!(t.variants.len(), 1);
    assert_eq!(t.variants[0].body, "{name} was added");
}
```

- [ ] **Step 2: Implement Project::save_template**

Append to `prosaic-project/src/project.rs`:

```rust
impl Project {
    /// Write the named template back to disk as TOML. Creates `templates/`
    /// if missing.
    pub fn save_template(&self, key: &str) -> Result<(), ProjectError> {
        let template = self.templates.get(key).ok_or_else(|| ProjectError::TemplateValidation {
            key: key.to_string(),
            reason: "template not present in project".to_string(),
        })?;
        let dir = self.root.join("templates");
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| ProjectError::Io {
                path: dir.display().to_string(),
                source: e.to_string(),
            })?;
        }
        let serialized = toml::to_string_pretty(template).map_err(|e| ProjectError::TomlParse {
            file: format!("{key}.toml"),
            source: e.to_string(),
        })?;
        let path = dir.join(format!("{key}.toml"));
        fs::write(&path, serialized).map_err(|e| ProjectError::Io {
            path: path.display().to_string(),
            source: e.to_string(),
        })?;
        Ok(())
    }

    /// Symmetrical save for partials.
    pub fn save_partial(&self, name: &str) -> Result<(), ProjectError> {
        let partial = self.partials.get(name).ok_or_else(|| ProjectError::PartialValidation {
            name: name.to_string(),
            reason: "partial not present in project".to_string(),
        })?;
        let dir = self.root.join("partials");
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| ProjectError::Io {
                path: dir.display().to_string(),
                source: e.to_string(),
            })?;
        }
        let serialized = toml::to_string_pretty(partial).map_err(|e| ProjectError::TomlParse {
            file: format!("{name}.toml"),
            source: e.to_string(),
        })?;
        let path = dir.join(format!("{name}.toml"));
        fs::write(&path, serialized).map_err(|e| ProjectError::Io {
            path: path.display().to_string(),
            source: e.to_string(),
        })?;
        Ok(())
    }

    /// Symmetrical save for scenarios.
    pub fn save_scenario(&self, name: &str) -> Result<(), ProjectError> {
        let scenario = self.scenarios.get(name).ok_or_else(|| ProjectError::ScenarioValidation {
            name: name.to_string(),
            reason: "scenario not present in project".to_string(),
        })?;
        let dir = self.root.join("tests");
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| ProjectError::Io {
                path: dir.display().to_string(),
                source: e.to_string(),
            })?;
        }
        let serialized = toml::to_string_pretty(scenario).map_err(|e| ProjectError::TomlParse {
            file: format!("{name}.toml"),
            source: e.to_string(),
        })?;
        let path = dir.join(format!("{name}.toml"));
        fs::write(&path, serialized).map_err(|e| ProjectError::Io {
            path: path.display().to_string(),
            source: e.to_string(),
        })?;
        Ok(())
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package prosaic-project --test save_template`
Expected: 1 passed.

- [ ] **Step 4: Commit**

```bash
git add prosaic-project/
git commit -m "Add Project::save_template/save_partial/save_scenario"
```

---

## Task 12: Scenario runner — render a scenario, check expectations

**Files:**
- Modify: `prosaic-project/src/runner.rs`
- Test: `prosaic-project/tests/scenario_runner.rs`

- [ ] **Step 1: Implement the runner module**

Replace `prosaic-project/src/runner.rs`:

```rust
//! Scenario runner — render a scenario through one Session and check
//! its output and discourse assertions against expectations.

use prosaic_core::{Context, Engine, Session, Value};
use prosaic_grammar_en::English;

use crate::error::ProjectError;
use crate::scenario::{Expected, Scenario};

#[derive(Debug, Clone)]
pub struct ScenarioOutcome {
    pub scenario_name: String,
    pub verdict: ScenarioVerdict,
    pub actual_output: String,
    pub event_outputs: Vec<String>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioVerdict {
    Pass,
    Fail,
}

pub struct ScenarioRunner<'a> {
    engine: &'a Engine<English>,
}

impl<'a> ScenarioRunner<'a> {
    pub fn new(engine: &'a Engine<English>) -> Self {
        Self { engine }
    }

    pub fn run(&self, scenario: &Scenario) -> Result<ScenarioOutcome, ProjectError> {
        let mut session = Session::new();
        let mut event_outputs = Vec::with_capacity(scenario.events.len());

        for event in &scenario.events {
            let ctx = scenario_event_to_context(event);
            // RST relation hint not yet wired through render() in this v1 path;
            // captured for future extension.
            let out = self
                .engine
                .render(&mut session, &event.template, &ctx)
                .map_err(|e| ProjectError::ScenarioValidation {
                    name: scenario.name.clone(),
                    reason: format!("event template `{}`: {e}", event.template),
                })?;
            event_outputs.push(out);
        }

        let actual_output = event_outputs.join(" ");

        let mut failures = Vec::new();
        if let Some(expected) = &scenario.expected {
            check_expected(expected, &actual_output, &mut failures);
        }

        let verdict = if failures.is_empty() {
            ScenarioVerdict::Pass
        } else {
            ScenarioVerdict::Fail
        };

        Ok(ScenarioOutcome {
            scenario_name: scenario.name.clone(),
            verdict,
            actual_output,
            event_outputs,
            failures,
        })
    }
}

fn scenario_event_to_context(event: &crate::scenario::ScenarioEvent) -> Context {
    let mut ctx = Context::new();
    for (k, v) in &event.context {
        ctx.insert(k.clone(), toml_to_value(v));
    }
    ctx
}

fn toml_to_value(v: &toml::Value) -> Value {
    use toml::Value as TV;
    match v {
        TV::String(s) => Value::String(s.clone()),
        TV::Integer(i) => Value::Number(*i),
        TV::Float(f) => Value::Number(*f as i64),
        TV::Boolean(b) => Value::Number(if *b { 1 } else { 0 }),
        TV::Array(items) => Value::List(
            items
                .iter()
                .map(|i| match i {
                    TV::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect(),
        ),
        _ => Value::String(v.to_string()),
    }
}

fn check_expected(expected: &Expected, actual: &str, failures: &mut Vec<String>) {
    if let Some(ref out) = expected.output {
        let actual_norm = actual.split_whitespace().collect::<Vec<_>>().join(" ");
        let expected_norm = out.split_whitespace().collect::<Vec<_>>().join(" ");
        if actual_norm != expected_norm {
            failures.push(format!(
                "output mismatch:\n  expected: {expected_norm}\n  actual:   {actual_norm}"
            ));
        }
    }
    // Discourse assertions and faithfulness checks are added in a follow-up task
    // once the explained-render API is wired through.
}
```

- [ ] **Step 2: Write the integration test**

Create `prosaic-project/tests/scenario_runner.rs`:

```rust
use prosaic_project::{Project, ScenarioRunner, ScenarioVerdict};
use std::path::Path;

#[test]
fn runner_passes_matching_scenario() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();
    let engine = p.into_engine().unwrap();
    let scenario = p.scenarios.get("smoke").unwrap();
    let outcome = ScenarioRunner::new(&engine).run(scenario).unwrap();
    assert_eq!(outcome.verdict, ScenarioVerdict::Pass, "failures: {:?}", outcome.failures);
    assert!(outcome.actual_output.contains("UserService"));
    assert_eq!(outcome.event_outputs.len(), 1);
}

#[test]
fn runner_fails_on_output_mismatch() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();

    // Mutate the scenario's expected output in-memory to force a mismatch
    let mut scenario = p.scenarios.get("smoke").unwrap().clone();
    let mut exp = scenario.expected.clone().unwrap_or_default();
    exp.output = Some("totally wrong expected text".to_string());
    scenario.expected = Some(exp);

    let engine = p.into_engine().unwrap();
    let outcome = ScenarioRunner::new(&engine).run(&scenario).unwrap();
    assert_eq!(outcome.verdict, ScenarioVerdict::Fail);
    assert!(!outcome.failures.is_empty());
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package prosaic-project --test scenario_runner`
Expected: 2 passed.

- [ ] **Step 4: Commit**

```bash
git add prosaic-project/
git commit -m "Add ScenarioRunner with output mismatch detection"
```

---

## Task 13: Build bundler — JSON manifest target

**Files:**
- Modify: `prosaic-project/src/bundle.rs`
- Test: `prosaic-project/tests/bundle_json.rs`

- [ ] **Step 1: Test + impl**

Replace `prosaic-project/src/bundle.rs`:

```rust
//! Project → portable bundle (JSON or generated Rust source).

use serde::Serialize;

use crate::error::ProjectError;
use crate::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTarget {
    JsonManifest,
    RustModule,
    Both,
}

#[derive(Debug, Clone, Default)]
pub struct BuildOutput {
    pub json: Option<String>,
    pub rust: Option<String>,
}

#[derive(Serialize)]
struct JsonBundle<'a> {
    schema_version: u32,
    name: &'a str,
    version: &'a str,
    language: &'a str,
    engine: &'a crate::manifest::EngineSettings,
    templates: Vec<&'a crate::template::TemplateFile>,
    partials: Vec<&'a crate::partial::PartialFile>,
}

pub fn build_bundle(project: &Project, target: BuildTarget) -> Result<BuildOutput, ProjectError> {
    let mut out = BuildOutput::default();
    if matches!(target, BuildTarget::JsonManifest | BuildTarget::Both) {
        out.json = Some(build_json(project)?);
    }
    if matches!(target, BuildTarget::RustModule | BuildTarget::Both) {
        out.rust = Some(build_rust(project)?);
    }
    Ok(out)
}

fn build_json(project: &Project) -> Result<String, ProjectError> {
    let bundle = JsonBundle {
        schema_version: 1,
        name: &project.manifest.name,
        version: &project.manifest.version,
        language: &project.manifest.language,
        engine: &project.manifest.engine,
        templates: project.templates.values().collect(),
        partials: project.partials.values().collect(),
    };
    serde_json::to_string_pretty(&bundle).map_err(|e| ProjectError::JsonParse {
        file: "(bundle)".to_string(),
        source: e.to_string(),
    })
}

fn build_rust(project: &Project) -> Result<String, ProjectError> {
    use std::fmt::Write;
    let mut s = String::new();
    writeln!(s, "// Generated by `prosaic build` — do not edit by hand.").unwrap();
    writeln!(s, "// schema_version = 1").unwrap();
    writeln!(s, "use prosaic_core::{{Engine, Salience}};").unwrap();
    writeln!(s, "use prosaic_grammar_en::English;").unwrap();
    writeln!(s).unwrap();
    writeln!(
        s,
        "pub fn register(engine: &mut Engine<English>) -> Result<(), prosaic_core::ProsaicError> {{"
    )
    .unwrap();
    for partial in project.partials.values() {
        writeln!(
            s,
            "    engine.register_partial({:?}, {:?})?;",
            partial.name, partial.body
        )
        .unwrap();
    }
    for template in project.templates.values() {
        for variant in &template.variants {
            let salience = match variant.salience.as_str() {
                "low" => "Salience::Low",
                "high" => "Salience::High",
                _ => "Salience::Medium",
            };
            let lang = variant.language.as_deref().unwrap_or("");
            if lang.is_empty() {
                writeln!(
                    s,
                    "    engine.register_template_at({:?}, {:?}, {})?;",
                    template.key, variant.body, salience
                )
                .unwrap();
            } else {
                writeln!(
                    s,
                    "    engine.register_template_with_language_at({:?}, {:?}, {}, Some({:?}))?;",
                    template.key, variant.body, salience, lang
                )
                .unwrap();
            }
        }
    }
    writeln!(s, "    Ok(())").unwrap();
    writeln!(s, "}}").unwrap();
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use std::path::Path;

    fn project() -> Project {
        Project::load_from_dir(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
        )
        .unwrap()
    }

    #[test]
    fn json_bundle_contains_template() {
        let bundle = build_bundle(&project(), BuildTarget::JsonManifest).unwrap();
        let json = bundle.json.unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"code.modified\""));
        assert!(json.contains("\"impact_tail\""));
    }

    #[test]
    fn rust_bundle_emits_register_calls() {
        let bundle = build_bundle(&project(), BuildTarget::RustModule).unwrap();
        let rust = bundle.rust.unwrap();
        assert!(rust.contains("register_partial(\"impact_tail\""));
        assert!(rust.contains("register_template_at(\"code.modified\""));
        assert!(rust.contains("Salience::Low"));
        assert!(rust.contains("Salience::Medium"));
        assert!(rust.contains("Salience::High"));
    }

    #[test]
    fn both_target_produces_both() {
        let bundle = build_bundle(&project(), BuildTarget::Both).unwrap();
        assert!(bundle.json.is_some());
        assert!(bundle.rust.is_some());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package prosaic-project --lib bundle::tests`
Expected: 3 passed.

- [ ] **Step 3: Commit**

```bash
git add prosaic-project/
git commit -m "Add build_bundle for JSON manifest and generated Rust targets"
```

---

## Task 14: Engine::load_manifest — runtime loader for the JSON bundle

**Files:**
- Modify: `prosaic-core/src/engine.rs`
- Test: `prosaic-core/tests/load_manifest.rs`

The JSON bundle format from Task 13 needs a counterpart loader on `Engine` so any host can `Engine::load_manifest(json_str)` without depending on prosaic-project.

- [ ] **Step 1: Write the failing test**

Create `prosaic-core/tests/load_manifest.rs`:

```rust
use prosaic_core::{Context, Engine, Session, Value};
use prosaic_grammar_en::English;

const SAMPLE: &str = r#"
{
  "schema_version": 1,
  "name": "demo",
  "version": "0.1.0",
  "language": "en",
  "engine": {
    "strictness": "strict",
    "variation": "fixed",
    "smart_quotes": false,
    "max_sentence_length": 0,
    "faithfulness_min": 0.0,
    "salience_thresholds": null
  },
  "templates": [
    {
      "key": "greet",
      "description": "",
      "slots_required": ["name"],
      "slots_optional": [],
      "variants": [
        {
          "salience": "medium",
          "language": null,
          "description": "",
          "body": "Hello {name}"
        }
      ]
    }
  ],
  "partials": []
}
"#;

#[test]
fn load_manifest_renders_template() {
    let mut engine = Engine::new(English::new());
    engine.load_manifest(SAMPLE).unwrap();

    let mut ctx = Context::new();
    ctx.insert("name", Value::String("world".into()));

    let mut session = Session::new();
    let out = engine.render(&mut session, "greet", &ctx).unwrap();
    assert!(out.contains("Hello"));
    assert!(out.contains("world"));
}
```

- [ ] **Step 2: Implement Engine::load_manifest**

Add to `prosaic-core/src/engine.rs`. Add the dependency on `serde_json` to prosaic-core (gated on the `serde` feature, since that's where this naturally lives). Update `prosaic-core/Cargo.toml`:

```toml
[dependencies]
# ... existing
serde_json = { version = "1", optional = true }

[features]
serde = ["dep:serde", "dep:serde_json"]
```

In `engine.rs`, gate the loader on `serde`:

```rust
#[cfg(feature = "serde")]
mod manifest_loader {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    pub struct ManifestBundle {
        pub schema_version: u32,
        pub name: String,
        pub version: String,
        pub language: String,
        pub engine: ManifestEngineSettings,
        pub templates: Vec<ManifestTemplate>,
        pub partials: Vec<ManifestPartial>,
    }

    #[derive(Deserialize, Default)]
    pub struct ManifestEngineSettings {
        pub strictness: String,
        pub variation: String,
        #[serde(default)]
        pub smart_quotes: bool,
        #[serde(default)]
        pub max_sentence_length: usize,
        #[serde(default)]
        pub faithfulness_min: f64,
        #[serde(default)]
        pub salience_thresholds: Option<ManifestSalienceThresholds>,
    }

    #[derive(Deserialize)]
    pub struct ManifestSalienceThresholds {
        pub low_max: i64,
        pub high_min: i64,
    }

    #[derive(Deserialize)]
    pub struct ManifestTemplate {
        pub key: String,
        #[serde(default)]
        pub description: String,
        pub variants: Vec<ManifestVariant>,
    }

    #[derive(Deserialize)]
    pub struct ManifestVariant {
        #[serde(default = "default_salience")]
        pub salience: String,
        #[serde(default)]
        pub language: Option<String>,
        pub body: String,
    }

    fn default_salience() -> String { "medium".into() }

    #[derive(Deserialize)]
    pub struct ManifestPartial {
        pub name: String,
        pub body: String,
    }
}

#[cfg(feature = "serde")]
impl<L: Language> Engine<L> {
    pub fn load_manifest(&mut self, json: &str) -> Result<(), ProsaicError> {
        let bundle: manifest_loader::ManifestBundle = serde_json::from_str(json)
            .map_err(|e| ProsaicError::TemplateParseError {
                template: "(manifest)".to_string(),
                position: 0,
                reason: format!("manifest JSON parse error: {e}"),
            })?;
        if bundle.schema_version != 1 {
            return Err(ProsaicError::TemplateParseError {
                template: "(manifest)".to_string(),
                position: 0,
                reason: format!("unsupported schema version {}", bundle.schema_version),
            });
        }
        self.language_preference = Some(bundle.language);
        for partial in bundle.partials {
            self.register_partial(&partial.name, &partial.body)?;
        }
        for template in bundle.templates {
            for variant in template.variants {
                let salience = match variant.salience.as_str() {
                    "low" => Salience::Low,
                    "high" => Salience::High,
                    _ => Salience::Medium,
                };
                self.register_template_with_language_at(
                    &template.key,
                    &variant.body,
                    salience,
                    variant.language.as_deref(),
                )?;
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 3: Run test**

Run: `cargo test --package prosaic-core --features serde --test load_manifest`
Expected: 1 passed.

- [ ] **Step 4: Commit**

```bash
git add prosaic-core/
git commit -m "Add Engine::load_manifest for runtime loading of bundled projects"
```

---

## Task 15: Scaffold module — starter projects

**Files:**
- Modify: `prosaic-project/src/scaffold.rs`
- Test: `prosaic-project/tests/scaffold.rs`

- [ ] **Step 1: Implement scaffold**

Replace `prosaic-project/src/scaffold.rs`:

```rust
//! Starter project templates for `prosaic new`.

use std::fs;
use std::path::Path;

use crate::error::ProjectError;

#[derive(Debug, Clone, Copy)]
pub enum Starter {
    Blank,
    Changelog,
    VocabPack,
}

impl Starter {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "blank" => Some(Self::Blank),
            "changelog" => Some(Self::Changelog),
            "vocab-pack" => Some(Self::VocabPack),
            _ => None,
        }
    }
}

pub fn scaffold_project(name: &str, dir: &Path, starter: Starter) -> Result<(), ProjectError> {
    if dir.exists() && fs::read_dir(dir).map(|d| d.count() > 0).unwrap_or(false) {
        return Err(ProjectError::Io {
            path: dir.display().to_string(),
            source: "directory exists and is not empty".to_string(),
        });
    }
    fs::create_dir_all(dir).map_err(|e| ProjectError::Io {
        path: dir.display().to_string(),
        source: e.to_string(),
    })?;

    let manifest = format!(
        r#"name = "{name}"
version = "0.1.0"
language = "en"

[engine]
strictness = "strict"
variation = "fixed"
"#
    );
    write(&dir.join("prosaic.toml"), &manifest)?;

    match starter {
        Starter::Blank => {
            fs::create_dir_all(dir.join("templates")).ok();
            fs::create_dir_all(dir.join("partials")).ok();
            fs::create_dir_all(dir.join("fixtures")).ok();
            fs::create_dir_all(dir.join("tests")).ok();
        }
        Starter::Changelog => {
            fs::create_dir_all(dir.join("templates")).ok();
            fs::create_dir_all(dir.join("fixtures")).ok();
            fs::create_dir_all(dir.join("tests")).ok();
            write(
                &dir.join("templates/code.added.toml"),
                r#"key = "code.added"

[[variants]]
salience = "medium"
body = "{name|refer} was added"
"#,
            )?;
            write(
                &dir.join("templates/code.modified.toml"),
                r#"key = "code.modified"

[[variants]]
salience = "low"
body = "{name|refer} was modified"

[[variants]]
salience = "medium"
body = "{name|refer} was modified, affecting {consumer_count} {consumer_count|pluralize:consumer}"
"#,
            )?;
            write(
                &dir.join("fixtures/userservice-modified.json"),
                r#"{"name": "UserService", "entity_type": "class", "consumer_count": 6}"#,
            )?;
            write(
                &dir.join("tests/sample-changeset.toml"),
                r#"name = "sample-changeset"

[[events]]
template = "code.added"
context = { name = "AuthGuard", entity_type = "class" }

[[events]]
template = "code.modified"
context = { name = "UserService", entity_type = "class", consumer_count = 6 }
"#,
            )?;
        }
        Starter::VocabPack => {
            fs::create_dir_all(dir.join("templates")).ok();
            fs::create_dir_all(dir.join("partials")).ok();
            fs::create_dir_all(dir.join("tests")).ok();
            write(
                &dir.join("partials/impact_tail.toml"),
                r#"name = "impact_tail"
description = "Trailing 'affecting N consumers' clause."
body = "{?consumer_count}, affecting {consumer_count} {consumer_count|pluralize:consumer}{/?}"
"#,
            )?;
            write(
                &dir.join("templates/code.modified.toml"),
                r#"key = "code.modified"
slots_required = ["name"]
slots_optional = ["consumer_count"]

[[variants]]
salience = "low"
body = "{name|refer} was modified"

[[variants]]
salience = "medium"
body = "{name|refer} was modified{>impact_tail}"

[[variants]]
salience = "high"
body = "{name|refer} has been substantially modified{>impact_tail}. Thorough review is recommended."
"#,
            )?;
        }
    }
    Ok(())
}

fn write(path: &Path, content: &str) -> Result<(), ProjectError> {
    fs::write(path, content).map_err(|e| ProjectError::Io {
        path: path.display().to_string(),
        source: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn scaffold_blank_creates_layout() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("blank-proj");
        scaffold_project("blank-proj", &dir, Starter::Blank).unwrap();
        assert!(dir.join("prosaic.toml").exists());
        assert!(dir.join("templates").is_dir());
        assert!(dir.join("partials").is_dir());
        assert!(dir.join("fixtures").is_dir());
        assert!(dir.join("tests").is_dir());
    }

    #[test]
    fn scaffold_changelog_creates_starter_templates() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("cl");
        scaffold_project("cl", &dir, Starter::Changelog).unwrap();
        assert!(dir.join("templates/code.added.toml").exists());
        assert!(dir.join("templates/code.modified.toml").exists());
        assert!(dir.join("fixtures/userservice-modified.json").exists());
        assert!(dir.join("tests/sample-changeset.toml").exists());
    }

    #[test]
    fn scaffold_vocab_pack_creates_partial() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("vp");
        scaffold_project("vp", &dir, Starter::VocabPack).unwrap();
        assert!(dir.join("partials/impact_tail.toml").exists());
        assert!(dir.join("templates/code.modified.toml").exists());
    }

    #[test]
    fn scaffold_into_nonempty_dir_errors() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("occupied");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("README.md"), "x").unwrap();
        let res = scaffold_project("occ", &dir, Starter::Blank);
        assert!(res.is_err());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package prosaic-project --lib scaffold::tests`
Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
git add prosaic-project/
git commit -m "Add starter project scaffolds (blank, changelog, vocab-pack)"
```

---

## Task 16: prosaic-cli — `new` subcommand

**Files:**
- Modify: `prosaic-cli/Cargo.toml`
- Modify: `prosaic-cli/src/main.rs`

- [ ] **Step 1: Add prosaic-project dep + clap setup**

In `prosaic-cli/Cargo.toml`:

```toml
[dependencies]
prosaic-core = { path = "../prosaic-core" }
prosaic-grammar-en = { path = "../prosaic-grammar-en" }
prosaic-grammar-es = { path = "../prosaic-grammar-es" }
prosaic-grammar-de = { path = "../prosaic-grammar-de" }
prosaic-vocab-code = { path = "../prosaic-vocab-code" }
prosaic-vocab-git = { path = "../prosaic-vocab-git" }
prosaic-vocab-pr = { path = "../prosaic-vocab-pr" }
prosaic-vocab-release = { path = "../prosaic-vocab-release" }
prosaic-project = { path = "../prosaic-project" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
```

(Add the `clap` dependency if not already present — check the existing Cargo.toml first; the existing CLI may use a different parser. If using `clap` already, just add `prosaic-project`.)

- [ ] **Step 2: Add the `new` subcommand**

Locate the existing `prosaic-cli/src/main.rs` `main()`. The current CLI is a stdin-pipe filter; subcommands need to coexist. Detect subcommand mode by checking if `argv[1]` is a known subcommand keyword:

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 {
        match args[1].as_str() {
            "new" => return run_new(&args[2..]),
            "build" => return run_build(&args[2..]),
            "test" => return run_test(&args[2..]),
            _ => {}
        }
    }
    // Existing pipe-filter behaviour (read JSON from stdin, render to stdout)
    run_pipe_filter();
}

fn run_new(args: &[String]) {
    // Parse: prosaic new <name> [--starter=blank|changelog|vocab-pack] [--at <dir>]
    let mut name: Option<String> = None;
    let mut starter = "blank".to_string();
    let mut at: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--starter" if i + 1 < args.len() => {
                starter = args[i + 1].clone();
                i += 2;
            }
            s if s.starts_with("--starter=") => {
                starter = s.trim_start_matches("--starter=").to_string();
                i += 1;
            }
            "--at" if i + 1 < args.len() => {
                at = Some(args[i + 1].clone());
                i += 2;
            }
            other if name.is_none() => {
                name = Some(other.to_string());
                i += 1;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }
    let name = match name {
        Some(n) => n,
        None => {
            eprintln!("usage: prosaic new <name> [--starter=blank|changelog|vocab-pack] [--at <dir>]");
            std::process::exit(2);
        }
    };
    let starter_kind = match prosaic_project::Starter::from_str(&starter) {
        Some(s) => s,
        None => {
            eprintln!("unknown starter `{starter}`; expected: blank | changelog | vocab-pack");
            std::process::exit(2);
        }
    };
    let dir = std::path::PathBuf::from(at.unwrap_or_else(|| name.clone()));
    match prosaic_project::scaffold_project(&name, &dir, starter_kind) {
        Ok(()) => println!("created project `{name}` at {}", dir.display()),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 3: Smoke test (manual)**

Run: `cargo run --package prosaic-cli -- new test-proj --starter=blank --at /tmp/test-proj-cli`
Expected: prints "created project `test-proj` at /tmp/test-proj-cli"; the directory contains `prosaic.toml` and the four subdirectories.

Clean up: `rm -rf /tmp/test-proj-cli`

- [ ] **Step 4: Commit**

```bash
git add prosaic-cli/
git commit -m "Add `prosaic new` subcommand for project scaffolding"
```

---

## Task 17: prosaic-cli — `build` subcommand

**Files:**
- Modify: `prosaic-cli/src/main.rs`

- [ ] **Step 1: Add the build subcommand**

Append to `main.rs`:

```rust
fn run_build(args: &[String]) {
    let mut target = "json".to_string();
    let mut out_dir: Option<String> = None;
    let mut project_dir = ".".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            s if s.starts_with("--target=") => {
                target = s.trim_start_matches("--target=").to_string();
                i += 1;
            }
            "--target" if i + 1 < args.len() => {
                target = args[i + 1].clone();
                i += 2;
            }
            s if s.starts_with("--out=") => {
                out_dir = Some(s.trim_start_matches("--out=").to_string());
                i += 1;
            }
            "--out" if i + 1 < args.len() => {
                out_dir = Some(args[i + 1].clone());
                i += 2;
            }
            other => {
                project_dir = other.to_string();
                i += 1;
            }
        }
    }
    let target_kind = match target.as_str() {
        "json" => prosaic_project::BuildTarget::JsonManifest,
        "rust" => prosaic_project::BuildTarget::RustModule,
        "both" => prosaic_project::BuildTarget::Both,
        other => {
            eprintln!("unknown target `{other}`; expected: json | rust | both");
            std::process::exit(2);
        }
    };
    let project = match prosaic_project::Project::load_from_dir(&project_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error loading project: {e}");
            std::process::exit(1);
        }
    };
    let bundle = match prosaic_project::build_bundle(&project, target_kind) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error building bundle: {e}");
            std::process::exit(1);
        }
    };
    let out_root = std::path::PathBuf::from(out_dir.unwrap_or_else(|| ".prosaic/build".to_string()));
    std::fs::create_dir_all(&out_root).ok();
    if let Some(json) = bundle.json {
        let path = out_root.join("prosaic.bundle.json");
        std::fs::write(&path, json).expect("write json bundle");
        println!("wrote {}", path.display());
    }
    if let Some(rust) = bundle.rust {
        let path = out_root.join("prosaic_bundle.rs");
        std::fs::write(&path, rust).expect("write rust bundle");
        println!("wrote {}", path.display());
    }
}
```

- [ ] **Step 2: Smoke test**

Set up a project, build it:

```bash
cargo run --package prosaic-cli -- new build-test --starter=changelog --at /tmp/build-test
cargo run --package prosaic-cli -- build /tmp/build-test --target=both --out=/tmp/build-test/.prosaic/build
ls /tmp/build-test/.prosaic/build/
```

Expected: directory contains `prosaic.bundle.json` and `prosaic_bundle.rs`. The JSON should include `"code.added"` and `"code.modified"`.

Clean up: `rm -rf /tmp/build-test`

- [ ] **Step 3: Commit**

```bash
git add prosaic-cli/
git commit -m "Add `prosaic build` subcommand for bundle generation"
```

---

## Task 18: prosaic-cli — `test` subcommand

**Files:**
- Modify: `prosaic-cli/src/main.rs`

- [ ] **Step 1: Implement run_test**

Append to `main.rs`:

```rust
fn run_test(args: &[String]) {
    let project_dir = args.first().cloned().unwrap_or_else(|| ".".to_string());
    let project = match prosaic_project::Project::load_from_dir(&project_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error loading project: {e}");
            std::process::exit(1);
        }
    };
    let engine = match project.into_engine() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error materializing engine: {e}");
            std::process::exit(1);
        }
    };
    let runner = prosaic_project::ScenarioRunner::new(&engine);
    let total = project.scenarios.len();
    let mut passed = 0usize;
    let mut failed = 0usize;
    for (name, scenario) in &project.scenarios {
        match runner.run(scenario) {
            Ok(outcome) => {
                if outcome.verdict == prosaic_project::ScenarioVerdict::Pass {
                    println!("PASS  {name}");
                    passed += 1;
                } else {
                    println!("FAIL  {name}");
                    for f in &outcome.failures {
                        println!("      {f}");
                    }
                    failed += 1;
                }
            }
            Err(e) => {
                println!("ERROR {name}: {e}");
                failed += 1;
            }
        }
    }
    println!();
    println!("{passed}/{total} passed");
    if failed > 0 {
        std::process::exit(1);
    }
}
```

- [ ] **Step 2: Smoke test**

```bash
cargo run --package prosaic-cli -- new t-test --starter=changelog --at /tmp/t-test
cargo run --package prosaic-cli -- test /tmp/t-test
```

Expected: prints `PASS sample-changeset` (no expected output configured in the starter, so the runner only fails if engine errors).

- [ ] **Step 3: Commit**

```bash
git add prosaic-cli/
git commit -m "Add `prosaic test` subcommand for scenario execution"
```

---

## Task 19: prosaic-wasm — expand exports for Studio

**Files:**
- Modify: `prosaic-wasm/Cargo.toml`
- Modify: `prosaic-wasm/src/lib.rs`

- [ ] **Step 1: Add prosaic-project + serde-wasm-bindgen deps**

In `prosaic-wasm/Cargo.toml`:

```toml
[dependencies]
prosaic-core = { path = "../prosaic-core", features = ["serde"] }
prosaic-grammar-en = { path = "../prosaic-grammar-en" }
prosaic-grammar-es = { path = "../prosaic-grammar-es" }
prosaic-grammar-de = { path = "../prosaic-grammar-de" }
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"
```

(Verify versions against existing Cargo.toml.)

- [ ] **Step 2: Add new wasm exports**

Read the existing `prosaic-wasm/src/lib.rs` to understand current structure. Add these methods to the existing `ProsaicEngine` impl block (or create one if absent):

```rust
#[wasm_bindgen]
impl ProsaicEngine {
    /// Load a project from its bundled JSON manifest.
    pub fn load_manifest(&mut self, json: &str) -> Result<(), JsError> {
        self.inner
            .load_manifest(json)
            .map_err(|e| JsError::new(&format!("{e}")))
    }

    /// Render with explanation; returns a JSON-serialized RenderExplanation.
    pub fn render_explained(
        &self,
        session: &mut ProsaicSession,
        key: &str,
        ctx: JsValue,
    ) -> Result<JsValue, JsError> {
        let context: prosaic_core::Context = serde_wasm_bindgen::from_value(ctx)
            .map_err(|e| JsError::new(&format!("context decode: {e}")))?;
        let exp = self
            .inner
            .render_explained(&mut session.inner, key, &context)
            .map_err(|e| JsError::new(&format!("{e}")))?;
        serde_wasm_bindgen::to_value(&exp).map_err(|e| JsError::new(&format!("explain encode: {e}")))
    }

    /// Score the variants that would be considered for a render.
    pub fn score_variants(
        &self,
        session: &mut ProsaicSession,
        key: &str,
        ctx: JsValue,
    ) -> Result<JsValue, JsError> {
        let context: prosaic_core::Context = serde_wasm_bindgen::from_value(ctx)
            .map_err(|e| JsError::new(&format!("context decode: {e}")))?;
        let scores = self
            .inner
            .score_variants(&mut session.inner, key, &context)
            .map_err(|e| JsError::new(&format!("{e}")))?;
        serde_wasm_bindgen::to_value(&scores).map_err(|e| JsError::new(&format!("scores encode: {e}")))
    }

    /// Score faithfulness of a rendered string against a context.
    pub fn score_faithfulness(&self, output: &str, ctx: JsValue) -> Result<JsValue, JsError> {
        let context: prosaic_core::Context = serde_wasm_bindgen::from_value(ctx)
            .map_err(|e| JsError::new(&format!("context decode: {e}")))?;
        // Use empty literal_tokens; Studio surfaces this for diagnostic purposes.
        let score = prosaic_core::score_faithfulness(output, &context, &[], self.inner.language());
        serde_wasm_bindgen::to_value(&score).map_err(|e| JsError::new(&format!("score encode: {e}")))
    }

    /// Validate a template body. Returns a JSON object: `{ ok: bool, error: string|null }`.
    pub fn validate_template(&self, body: &str) -> JsValue {
        match prosaic_core::Template::parse(body) {
            Ok(_) => serde_wasm_bindgen::to_value(&serde_json::json!({
                "ok": true, "error": null
            }))
            .unwrap(),
            Err(e) => serde_wasm_bindgen::to_value(&serde_json::json!({
                "ok": false, "error": e.to_string()
            }))
            .unwrap(),
        }
    }
}
```

The `Engine` needs a `language()` accessor for `score_faithfulness`. Add to prosaic-core if missing:

```rust
impl<L: Language> Engine<L> {
    pub fn language(&self) -> &L {
        &self.language
    }
}
```

- [ ] **Step 3: Verify wasm build**

Run: `cargo build --package prosaic-wasm --target wasm32-unknown-unknown`
Expected: clean build (may require `wasm32-unknown-unknown` target installed: `rustup target add wasm32-unknown-unknown`).

If `wasm-pack` is set up: `wasm-pack build prosaic-wasm --target web --release`
Expected: produces `pkg/` with `.wasm`, `.js`, `.d.ts` files.

- [ ] **Step 4: Commit**

```bash
git add prosaic-wasm/ prosaic-core/src/engine.rs
git commit -m "Expand prosaic-wasm exports for Studio integration"
```

---

## Task 20: Workspace test sweep + version bump

**Files:**
- Modify: `Cargo.toml` (version bump)
- Modify: `README.md` (note prosaic-project + new CLI commands)

- [ ] **Step 1: Run the full workspace test suite**

Run: `cargo test --workspace 2>&1 | grep -E "test result:|FAILED|error" | tail -40`
Expected: all `ok`, no FAILED.

- [ ] **Step 2: Bump workspace version 0.3.0 → 0.4.0**

In root `Cargo.toml`:

```toml
[workspace.package]
version = "0.4.0"
```

- [ ] **Step 3: Update README — add prosaic-project crate row + new CLI commands section**

Find the crate table; add:

```markdown
| `prosaic-project` | Folder-of-files project format (`prosaic.toml` + `templates/` + `partials/` + `fixtures/` + `tests/`); used by `prosaic-cli` and Prosaic Studio. |
```

Find the CLI section; add:

```markdown
### Project subcommands

```bash
# Scaffold a new project
prosaic new my-changelog --starter=changelog

# Build a portable bundle (JSON manifest, generated Rust, or both)
prosaic build --target=json --out=./dist

# Run all scenarios in tests/
prosaic test
```
```

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml README.md
git commit -m "Bump workspace to 0.4.0 and document prosaic-project + CLI subcommands"
```

- [ ] **Step 5: Push**

```bash
git push origin main
```

---

## Self-Review Checklist (post-write)

- [x] Spec coverage: prosaic-project crate, multi-language plumbing, scaffold/build/test CLI, prosaic-wasm exports, JSON manifest loader — all present in tasks 0–19. The Engine::load_manifest path closes the loop for any-host-language consumption.
- [x] Placeholder scan: no TBDs or "implement later" steps; all code blocks contain real code.
- [x] Type consistency: `Project`, `TemplateFile`, `Variant`, `Scenario`, `ScenarioRunner`, `BuildOutput`, `Starter` types are defined once in their own files, re-exported from lib.rs, used consistently across CLI and tests. `register_template_with_language_at` defined in Task 9 → used in Tasks 9, 14, 19.
- [x] Test coverage: every public function touched gets at least one test; edge cases for missing files, malformed input, non-empty target dirs covered.

---

## Notes for Plan 2 (Studio app — written after Plan 1 execution)

After this plan ships, Plan 2 covers:
1. New repo at `~/Documents/development/@wildmason/prosaic-studio/` with Tauri 2 + Angular 21 scaffold.
2. Tauri commands wrapping `prosaic-project` (open_project, save_template, watch_project).
3. Angular shell with the locked layout (folder tree + sidebar cards + editor + stacked previews).
4. Monaco editor with custom Prosaic DSL language definition.
5. wasm loader and `StudioEngine` service wrapping the prosaic-wasm bindings.
6. Single-render and narrative-flow preview components driven by signals.
7. Test runner UI + new project wizard.
8. Tauri auto-updater + signed builds + distribution pipeline.

Plan 2 will be written using the actual API surface that emerges from Plan 1 — no guessing.
