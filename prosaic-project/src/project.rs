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
            cause: e.to_string(),
        })?;
        let manifest: Manifest =
            toml::from_str(&manifest_str).map_err(|e| ProjectError::TomlParse {
                file: "prosaic.toml".to_string(),
                cause: e.to_string(),
            })?;

        let templates = load_toml_dir::<TemplateFile, _>(&root.join("templates"), |t| t.key.clone())?;
        let partials = load_toml_dir::<PartialFile, _>(&root.join("partials"), |p| p.name.clone())?;
        let scenarios = load_toml_dir::<Scenario, _>(&root.join("tests"), |s| s.name.clone())?;
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

fn load_toml_dir<T, F>(dir: &Path, key_fn: F) -> Result<HashMap<String, T>, ProjectError>
where
    T: serde::de::DeserializeOwned,
    F: Fn(&T) -> String,
{
    let mut out = HashMap::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir).map_err(|e| ProjectError::Io {
        path: dir.display().to_string(),
        cause: e.to_string(),
    })? {
        let entry = entry.map_err(|e| ProjectError::Io {
            path: dir.display().to_string(),
            cause: e.to_string(),
        })?;
        let path = entry.path();
        if path.extension().map(|e| e == "toml").unwrap_or(false) {
            let text = fs::read_to_string(&path).map_err(|e| ProjectError::Io {
                path: path.display().to_string(),
                cause: e.to_string(),
            })?;
            let parsed: T = toml::from_str(&text).map_err(|e| ProjectError::TomlParse {
                file: path.file_name().unwrap().to_string_lossy().to_string(),
                cause: e.to_string(),
            })?;
            let key = key_fn(&parsed);
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
        cause: e.to_string(),
    })? {
        let entry = entry.map_err(|e| ProjectError::Io {
            path: dir.display().to_string(),
            cause: e.to_string(),
        })?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let stem = path
                .file_stem()
                .ok_or_else(|| ProjectError::Io {
                    path: path.display().to_string(),
                    cause: "file has no stem".to_string(),
                })?
                .to_string_lossy()
                .to_string();
            let text = fs::read_to_string(&path).map_err(|e| ProjectError::Io {
                path: path.display().to_string(),
                cause: e.to_string(),
            })?;
            let ctx = parse_fixture(&stem, &text)?;
            out.insert(stem, ctx);
        }
    }
    Ok(out)
}
