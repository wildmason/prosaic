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

        let templates =
            load_toml_dir::<TemplateFile, _>(&root.join("templates"), |t| t.key.clone())?;
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

use prosaic_core::{Engine, Salience, SalienceThresholds, Strictness, Variation, pipe_spec};
use prosaic_grammar_en::English;

impl Project {
    /// Walk every template; report unknown pipes and unknown partial
    /// references as validation issues. Does not error.
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
                    if pipe_spec(&pipe_name).is_none() {
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

    /// Write the named template back to disk as TOML.
    pub fn save_template(&self, key: &str) -> Result<(), ProjectError> {
        let template = self
            .templates
            .get(key)
            .ok_or_else(|| ProjectError::TemplateValidation {
                key: key.to_string(),
                reason: "template not present in project".to_string(),
            })?;
        let dir = self.root.join("templates");
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| ProjectError::Io {
                path: dir.display().to_string(),
                cause: e.to_string(),
            })?;
        }
        let serialized = toml::to_string_pretty(template).map_err(|e| ProjectError::TomlParse {
            file: format!("{key}.toml"),
            cause: e.to_string(),
        })?;
        let path = dir.join(format!("{key}.toml"));
        fs::write(&path, serialized).map_err(|e| ProjectError::Io {
            path: path.display().to_string(),
            cause: e.to_string(),
        })
    }

    /// Write the named partial back to disk as TOML.
    pub fn save_partial(&self, name: &str) -> Result<(), ProjectError> {
        let partial = self
            .partials
            .get(name)
            .ok_or_else(|| ProjectError::PartialValidation {
                name: name.to_string(),
                reason: "partial not present in project".to_string(),
            })?;
        let dir = self.root.join("partials");
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| ProjectError::Io {
                path: dir.display().to_string(),
                cause: e.to_string(),
            })?;
        }
        let serialized = toml::to_string_pretty(partial).map_err(|e| ProjectError::TomlParse {
            file: format!("{name}.toml"),
            cause: e.to_string(),
        })?;
        let path = dir.join(format!("{name}.toml"));
        fs::write(&path, serialized).map_err(|e| ProjectError::Io {
            path: path.display().to_string(),
            cause: e.to_string(),
        })
    }

    /// Write the named scenario back to disk as TOML.
    pub fn save_scenario(&self, name: &str) -> Result<(), ProjectError> {
        let scenario =
            self.scenarios
                .get(name)
                .ok_or_else(|| ProjectError::ScenarioValidation {
                    name: name.to_string(),
                    reason: "scenario not present in project".to_string(),
                })?;
        let dir = self.root.join("tests");
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| ProjectError::Io {
                path: dir.display().to_string(),
                cause: e.to_string(),
            })?;
        }
        let serialized = toml::to_string_pretty(scenario).map_err(|e| ProjectError::TomlParse {
            file: format!("{name}.toml"),
            cause: e.to_string(),
        })?;
        let path = dir.join(format!("{name}.toml"));
        fs::write(&path, serialized).map_err(|e| ProjectError::Io {
            path: path.display().to_string(),
            cause: e.to_string(),
        })
    }
}

impl Project {
    /// Materialize a configured Engine from this project.
    /// v1: English only; multi-grammar selection by manifest is plumbed
    /// at the variant level (per-variant `language` field) but the
    /// engine itself is single-grammar in v1.
    pub fn into_engine(&self) -> Result<Engine, ProjectError> {
        let mut engine = Engine::new(English::new());

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
            engine = engine.with_faithfulness_gate(s.faithfulness_min as f32);
        }
        if let Some(thr) = &s.salience_thresholds {
            engine = engine.salience_thresholds(SalienceThresholds {
                low_max: thr.low_max,
                high_min: thr.high_min,
            });
        }
        if let Some(style) = &s.style {
            engine = engine.style_preference(style);
        }
        if let Some(profile_cfg) = &self.manifest.style_profile {
            let profile = profile_cfg.clone().into_style_profile(&self.root)?;
            engine = engine.style_profile(profile);
        }
        engine = engine.language_preference(&self.manifest.language);

        for (name, partial) in &self.partials {
            engine.register_partial(name, &partial.body).map_err(|e| {
                ProjectError::PartialValidation {
                    name: name.clone(),
                    reason: e.to_string(),
                }
            })?;
        }

        for (key, template) in &self.templates {
            for variant in &template.variants {
                let salience = match variant.salience.as_str() {
                    "low" => Salience::Low,
                    "medium" => Salience::Medium,
                    "high" => Salience::High,
                    other => {
                        return Err(ProjectError::TemplateValidation {
                            key: key.clone(),
                            reason: format!("unknown salience `{other}`"),
                        });
                    }
                };
                let language = variant.language.as_deref();
                let style = variant.style.as_deref();
                engine
                    .register_template_with_language_and_style_at(
                        key,
                        &variant.body,
                        salience,
                        language,
                        style,
                    )
                    .map_err(|e| ProjectError::TemplateValidation {
                        key: key.clone(),
                        reason: e.to_string(),
                    })?;
            }
        }

        Ok(engine)
    }
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
