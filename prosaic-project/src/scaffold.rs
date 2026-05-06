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

impl std::str::FromStr for Starter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "blank" => Ok(Self::Blank),
            "changelog" => Ok(Self::Changelog),
            "vocab-pack" => Ok(Self::VocabPack),
            other => Err(format!("unknown starter `{other}`; expected: blank | changelog | vocab-pack")),
        }
    }
}

pub fn scaffold_project(name: &str, dir: &Path, starter: Starter) -> Result<(), ProjectError> {
    if dir.exists() && fs::read_dir(dir).map(|d| d.count() > 0).unwrap_or(false) {
        return Err(ProjectError::Io {
            path: dir.display().to_string(),
            cause: "directory exists and is not empty".to_string(),
        });
    }
    fs::create_dir_all(dir).map_err(|e| ProjectError::Io {
        path: dir.display().to_string(),
        cause: e.to_string(),
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
body = "{name|refer} was modified{?consumer_count}, affecting {consumer_count} {consumer_count|pluralize:consumer}{/?}"
"#,
            )?;
            write(
                &dir.join("fixtures/userservice-modified.json"),
                r#"{"name": "UserService", "entity_type": "class", "consumer_count": 6}"#,
            )?;
            write(
                &dir.join("fixtures/authguard-added.json"),
                r#"{"name": "AuthGuard", "entity_type": "class"}"#,
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
            write(
                &dir.join("tests/authguard-added.toml"),
                r#"name = "authguard-added"

[[events]]
template = "code.added"
context = { name = "AuthGuard", entity_type = "class" }
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
        cause: e.to_string(),
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
        assert!(dir.join("fixtures/authguard-added.json").exists());
        assert!(dir.join("tests/sample-changeset.toml").exists());
        assert!(dir.join("tests/authguard-added.toml").exists());

        let project = crate::Project::load_from_dir(&dir).unwrap();
        assert_eq!(project.fixtures.len(), 2);
        assert_eq!(project.scenarios.len(), 2);

        let engine = project.into_engine().unwrap();
        let authguard = project.fixtures.get("authguard-added").unwrap();
        let mut session = prosaic_core::Session::new();
        let output = engine
            .render(&mut session, "code.modified", authguard)
            .unwrap();
        assert!(output.contains("AuthGuard"));
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
