# prosaic-project

Folder-of-files project format and bundler for Prosaic templates.

`prosaic-project` loads, validates, tests, and bundles Prosaic template projects.
It is the library behind the `prosaic new`, `prosaic build`, and `prosaic test`
CLI subcommands.

## Install

```toml
[dependencies]
prosaic-project = "1.0.1"
```

## Project Layout

```text
my-project/
  prosaic.toml
  templates/
  partials/
  fixtures/
  tests/
```

The manifest configures engine settings, vocabulary dependencies, style
preferences, and optional style profiles. Templates and partials are TOML files;
fixtures are JSON context maps; tests are scenario files that render one or more
events through a session and assert the resulting prose.

## What It Provides

- `Project::load_from_dir` for loading the folder format.
- Validation for template parse errors, missing fixtures, and pipe/schema issues.
- `Project::into_engine` for materializing a configured `prosaic_core::Engine`.
- Scenario execution through `ScenarioRunner`.
- `build_bundle` for portable JSON manifests or generated Rust source.
- Starter scaffolds for blank, changelog, and vocabulary-pack projects.
- A catalog of reference `StyleProfile` presets.

## Example

```rust
use prosaic_project::{BuildTarget, Project, build_bundle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::load_from_dir("my-project")?;
    let issues = project.validate();
    assert!(issues.is_empty());

    let bundle = build_bundle(&project, BuildTarget::Json)?;
    assert!(bundle.json.is_some());
    Ok(())
}
```

## License

MIT OR Apache-2.0
