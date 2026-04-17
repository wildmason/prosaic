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
    assert_eq!(
        outcome.verdict,
        ScenarioVerdict::Pass,
        "failures: {:?}",
        outcome.failures
    );
    assert!(outcome.actual_output.contains("UserService"));
    assert_eq!(outcome.event_outputs.len(), 1);
}

#[test]
fn runner_fails_on_output_mismatch() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();

    let mut scenario = p.scenarios.get("smoke").unwrap().clone();
    let mut exp = scenario.expected.clone().unwrap_or_default();
    exp.output = Some("totally wrong expected text".to_string());
    scenario.expected = Some(exp);

    let engine = p.into_engine().unwrap();
    let outcome = ScenarioRunner::new(&engine).run(&scenario).unwrap();
    assert_eq!(outcome.verdict, ScenarioVerdict::Fail);
    assert!(!outcome.failures.is_empty());
}
