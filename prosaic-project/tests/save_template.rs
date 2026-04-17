use prosaic_project::{Project, TemplateFile, Variant};
use tempfile::tempdir;

#[test]
fn save_template_round_trips() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    std::fs::write(
        root.join("prosaic.toml"),
        "name = \"tmp\"\nversion = \"0.1.0\"\nlanguage = \"en\"\n",
    )
    .unwrap();
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

    let reloaded = Project::load_from_dir(root).unwrap();
    let t = reloaded.templates.get("code.added").unwrap();
    assert_eq!(t.variants.len(), 1);
    assert_eq!(t.variants[0].body, "{name} was added");
}
