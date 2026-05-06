//! Scenario runner — render a scenario through one Session and check
//! its output and discourse assertions against expectations.

use prosaic_core::{
    Context, Engine, RenderExplanation, RstRelation, Session, Template, Value, score_faithfulness,
};

use crate::error::ProjectError;
use crate::scenario::{Expected, ExpectedDiscourse, Scenario, ScenarioEvent};

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
        reject_unsupported_engine_overrides(scenario)?;

        let mut event_data = Vec::with_capacity(scenario.events.len());
        for event in &scenario.events {
            let ctx = scenario_event_to_context(event);
            let relation = parse_rst_relation(&scenario.name, event)?;
            event_data.push((event.template.clone(), ctx, relation));
        }

        let mut explain_session = Session::new();
        let mut event_outputs = Vec::with_capacity(event_data.len());
        let mut explanations = Vec::with_capacity(event_data.len());
        let mut faithfulness_scores = Vec::with_capacity(event_data.len());

        for (template, ctx, _) in &event_data {
            let explanation = self
                .engine
                .render_explained(&mut explain_session, template, ctx)
                .map_err(|e| ProjectError::ScenarioValidation {
                    name: scenario.name.clone(),
                    reason: format!("event template `{template}`: {e}"),
                })?;
            let parsed_template = Template::parse(&explanation.variant_source).map_err(|e| {
                ProjectError::ScenarioValidation {
                    name: scenario.name.clone(),
                    reason: format!("selected template `{template}` could not be parsed: {e}"),
                }
            })?;
            let literals = parsed_template.literal_tokens();
            let score =
                score_faithfulness(&explanation.output, ctx, &literals, self.engine.language());
            event_outputs.push(explanation.output.clone());
            explanations.push(explanation);
            faithfulness_scores.push(score);
        }

        let actual_output = if event_data.iter().any(|(_, _, relation)| relation.is_some()) {
            let mut session = Session::new();
            let batch: Vec<(&str, Context, Option<RstRelation>)> = event_data
                .iter()
                .map(|(template, ctx, relation)| (template.as_str(), ctx.clone(), *relation))
                .collect();
            self.engine
                .render_batch_with_relations(&mut session, &batch)
                .map_err(|e| ProjectError::ScenarioValidation {
                    name: scenario.name.clone(),
                    reason: format!("rendering RST scenario: {e}"),
                })?
        } else {
            event_outputs.join(" ")
        };

        let mut failures = Vec::new();
        if let Some(expected) = &scenario.expected {
            check_expected(
                expected,
                &actual_output,
                &explanations,
                &faithfulness_scores,
                &mut failures,
            );
        }
        if let Some(min) = scenario.engine.faithfulness_min {
            check_faithfulness_min(min as f32, &faithfulness_scores, &mut failures);
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

fn reject_unsupported_engine_overrides(scenario: &Scenario) -> Result<(), ProjectError> {
    if scenario.engine.variation.is_some()
        || scenario.engine.language.is_some()
        || scenario.engine.salience_thresholds.is_some()
    {
        return Err(ProjectError::ScenarioValidation {
            name: scenario.name.clone(),
            reason: "scenario engine overrides for variation, language, and salience_thresholds are not supported by ScenarioRunner; configure the Project engine instead".to_string(),
        });
    }
    Ok(())
}

fn scenario_event_to_context(event: &ScenarioEvent) -> Context {
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

fn parse_rst_relation(
    scenario_name: &str,
    event: &ScenarioEvent,
) -> Result<Option<RstRelation>, ProjectError> {
    let Some(raw) = &event.rst_relation else {
        return Ok(None);
    };
    let normalized = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    let relation = match normalized.as_str() {
        "elaboration" => RstRelation::Elaboration,
        "contrast" => RstRelation::Contrast,
        "cause" => RstRelation::Cause,
        "result" => RstRelation::Result,
        "concession" => RstRelation::Concession,
        "sequence" => RstRelation::Sequence,
        "condition" => RstRelation::Condition,
        "background" => RstRelation::Background,
        "summary" => RstRelation::Summary,
        other => {
            return Err(ProjectError::ScenarioValidation {
                name: scenario_name.to_string(),
                reason: format!("unknown rst_relation `{other}`"),
            });
        }
    };
    Ok(Some(relation))
}

fn check_expected(
    expected: &Expected,
    actual: &str,
    explanations: &[RenderExplanation],
    faithfulness_scores: &[prosaic_core::FaithfulnessScore],
    failures: &mut Vec<String>,
) {
    if let Some(ref out) = expected.output {
        let actual_norm = actual.split_whitespace().collect::<Vec<_>>().join(" ");
        let expected_norm = out.split_whitespace().collect::<Vec<_>>().join(" ");
        if actual_norm != expected_norm {
            failures.push(format!(
                "output mismatch:\n  expected: {expected_norm}\n  actual:   {actual_norm}"
            ));
        }
    }
    if let Some(min) = expected.faithfulness_min {
        check_faithfulness_min(min as f32, faithfulness_scores, failures);
    }
    for discourse in &expected.discourse {
        check_expected_discourse(discourse, explanations, actual, failures);
    }
}

fn check_faithfulness_min(
    min: f32,
    scores: &[prosaic_core::FaithfulnessScore],
    failures: &mut Vec<String>,
) {
    for (idx, score) in scores.iter().enumerate() {
        if !score.passes(min) {
            failures.push(format!(
                "faithfulness below threshold at event {idx}: precision={:.3}, polarity_match={}, required={min:.3}, unentailed={:?}",
                score.precision, score.polarity_match, score.unentailed
            ));
        }
    }
}

fn check_expected_discourse(
    expected: &ExpectedDiscourse,
    explanations: &[RenderExplanation],
    actual_output: &str,
    failures: &mut Vec<String>,
) {
    let Some(explanation) = explanations.get(expected.event_index) else {
        failures.push(format!(
            "discourse expectation references missing event index {}",
            expected.event_index
        ));
        return;
    };

    if let Some(reference_form) = &expected.reference_form {
        let actual = explanation
            .reference_form
            .map(|form| format!("{form:?}"))
            .unwrap_or_else(|| "None".to_string());
        if !actual.eq_ignore_ascii_case(reference_form) {
            failures.push(format!(
                "reference_form mismatch at event {}: expected {reference_form}, actual {actual}",
                expected.event_index
            ));
        }
    }

    if let Some(needle) = &expected.connective_contains {
        let connective = explanation.connective.unwrap_or("");
        if !connective.contains(needle)
            && !explanation.output.contains(needle)
            && !actual_output.contains(needle)
        {
            failures.push(format!(
                "connective mismatch at event {}: expected text containing `{needle}`, actual connective={:?}, output={}",
                expected.event_index, explanation.connective, explanation.output
            ));
        }
    }

    if let Some(transition) = &expected.transition {
        let actual = format!("{:?}", explanation.centering_transition);
        if !actual.eq_ignore_ascii_case(transition) {
            failures.push(format!(
                "transition mismatch at event {}: expected {transition}, actual {actual}",
                expected.event_index
            ));
        }
    }
}
