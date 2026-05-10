//! Project loading and bundling errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("manifest missing at `{path}`: expected a `prosaic.toml` at the project root")]
    ManifestMissing { path: String },

    #[error("malformed TOML in `{file}`: {cause}")]
    TomlParse { file: String, cause: String },

    #[error("malformed JSON in `{file}`: {cause}")]
    JsonParse { file: String, cause: String },

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
    UnknownPipe {
        key: String,
        pipe: String,
        known: String,
    },

    #[error("io error reading `{path}`: {cause}")]
    Io { path: String, cause: String },

    #[error("style profile invalid: {reason}")]
    ManifestStyle { reason: String },

    #[error("engine error: {0}")]
    Engine(#[from] prosaic_core::ProsaicError),
}

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
            cause: "expected `=` after key".to_string(),
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
