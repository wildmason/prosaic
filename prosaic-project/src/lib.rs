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

pub use error::ProjectError;
pub use fixture::parse_fixture;
pub use manifest::{EngineSettings, Manifest, SalienceThresholdsConfig, VocabDependency};
pub use partial::PartialFile;
pub use scenario::{Expected, ExpectedDiscourse, Scenario, ScenarioEngineOverride, ScenarioEvent};
pub use template::{TemplateFile, Variant};
