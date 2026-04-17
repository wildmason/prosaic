//! Scenario runner — render a scenario through one Session and check
//! its output and discourse assertions against expectations.

use prosaic_core::{Context, Engine, Session, Value};

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
    engine: &'a Engine,
}

impl<'a> ScenarioRunner<'a> {
    pub fn new(engine: &'a Engine) -> Self {
        Self { engine }
    }

    pub fn run(&self, scenario: &Scenario) -> Result<ScenarioOutcome, ProjectError> {
        let mut session = Session::new();
        let mut event_outputs = Vec::with_capacity(scenario.events.len());

        for event in &scenario.events {
            let ctx = scenario_event_to_context(event);
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
}
