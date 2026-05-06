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

#[test]
fn runner_applies_rst_relation_to_expected_output() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();

    let mut scenario = p.scenarios.get("smoke").unwrap().clone();
    let mut second = scenario.events[0].clone();
    second.context.insert(
        "name".to_string(),
        toml::Value::String("AuthGuard".to_string()),
    );
    second.rst_relation = Some("elaboration".to_string());
    scenario.events.push(second);
    scenario.expected.as_mut().unwrap().output = Some(
        "UserService was modified, affecting 6. Furthermore, AuthGuard was modified, affecting 6."
            .to_string(),
    );

    let engine = p.into_engine().unwrap();
    let outcome = ScenarioRunner::new(&engine).run(&scenario).unwrap();
    assert_eq!(
        outcome.verdict,
        ScenarioVerdict::Pass,
        "failures: {:?}; actual={}",
        outcome.failures,
        outcome.actual_output
    );
}

#[test]
fn runner_checks_discourse_expectations() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();

    let mut scenario = p.scenarios.get("smoke").unwrap().clone();
    let exp = scenario.expected.as_mut().unwrap();
    exp.discourse.push(prosaic_project::ExpectedDiscourse {
        event_index: 0,
        reference_form: Some("Pronoun".to_string()),
        connective_contains: None,
        transition: None,
    });

    let engine = p.into_engine().unwrap();
    let outcome = ScenarioRunner::new(&engine).run(&scenario).unwrap();
    assert_eq!(outcome.verdict, ScenarioVerdict::Fail);
    assert!(
        outcome
            .failures
            .iter()
            .any(|f| f.contains("reference_form mismatch")),
        "failures: {:?}",
        outcome.failures
    );
}

#[test]
fn runner_checks_faithfulness_expectation() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();

    let mut scenario = p.scenarios.get("smoke").unwrap().clone();
    scenario.expected.as_mut().unwrap().faithfulness_min = Some(1.01);

    let engine = p.into_engine().unwrap();
    let outcome = ScenarioRunner::new(&engine).run(&scenario).unwrap();
    assert_eq!(outcome.verdict, ScenarioVerdict::Fail);
    assert!(
        outcome
            .failures
            .iter()
            .any(|f| f.contains("faithfulness below threshold")),
        "failures: {:?}",
        outcome.failures
    );
}

#[test]
fn runner_rejects_unsupported_engine_overrides() {
    let p = Project::load_from_dir(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/multi-variant"),
    )
    .unwrap();

    let mut scenario = p.scenarios.get("smoke").unwrap().clone();
    scenario.engine.variation = Some("round_robin".to_string());

    let engine = p.into_engine().unwrap();
    let err = ScenarioRunner::new(&engine).run(&scenario).unwrap_err();
    assert!(err.to_string().contains("engine overrides"));
}
