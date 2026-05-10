# Agent Narration

Agentic systems generate a lot of structured events as they work — tool calls, plan steps, decisions, retries, completions. The events are usually logged as JSON or emitted through `tracing`, and the human-readable summary is left to whatever the agent's LLM happens to produce on its way past. That summary is non-deterministic, occasionally hallucinated, and impossible to audit.

Prosaic is a natural fit for the narration layer. The agent emits structured events; Prosaic converts them into prose deterministically. The result is a narrative log that:

- says exactly what happened (every content token traces to a slot value or a template literal — see [Hallucination by construction](../../hallucination-by-construction.md));
- reads like writing, not like a log file;
- is regression-testable in CI alongside the agent's logic;
- never introduces new facts the agent didn't actually emit.

This cookbook entry covers two patterns: a direct synchronous call from the agent loop, and a `tracing`-bridged pattern where the agent emits events and `prosaic-tracing` narrates them in the background.

## Pattern A — Direct call from the agent loop

The agent emits a structured event for each step it takes. A small narration helper turns the event into prose via a registered Prosaic engine.

```toml
[dependencies]
prosaic-core = "0.6"
prosaic-grammar-en = "0.6"
```

```rust
use prosaic_core::{ctx, Engine, Session, Strictness, Variation, Value};
use prosaic_grammar_en::English;

#[derive(Clone)]
pub enum AgentEvent {
    PlanProposed { goal: String, steps: usize },
    ToolInvoked { tool: String, target: String },
    StepCompleted { step: usize, outcome: String },
    Replanned { reason: String, new_steps: usize },
    GoalReached { goal: String, total_steps: usize },
}

pub struct AgentNarrator {
    engine: Engine,
    session: Session,
}

impl AgentNarrator {
    pub fn new() -> Result<Self, prosaic_core::ProsaicError> {
        let mut engine = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Seeded(0xA6E47));

        engine.register_template(
            "agent.plan_proposed",
            "Proposed a plan to {goal} in {steps|words} {steps|pluralize:step}.",
        )?;
        engine.register_template(
            "agent.tool_invoked",
            "Invoked the {tool} tool against {target}.",
        )?;
        engine.register_template(
            "agent.step_completed",
            "Step {step|ordinal} completed: {outcome}.",
        )?;
        engine.register_template(
            "agent.replanned",
            "Replanned after {reason}, now {new_steps|words} {new_steps|pluralize:step}.",
        )?;
        engine.register_template(
            "agent.goal_reached",
            "Reached the goal — {goal} — after {total_steps|words} {total_steps|pluralize:step}.",
        )?;

        Ok(Self { engine, session: Session::new() })
    }

    pub fn narrate(&mut self, event: AgentEvent)
        -> Result<String, prosaic_core::ProsaicError>
    {
        let (key, ctx) = match event {
            AgentEvent::PlanProposed { goal, steps } => (
                "agent.plan_proposed",
                ctx! { goal: goal, steps: steps as i64 },
            ),
            AgentEvent::ToolInvoked { tool, target } => (
                "agent.tool_invoked",
                ctx! { tool: tool, target: target },
            ),
            AgentEvent::StepCompleted { step, outcome } => (
                "agent.step_completed",
                ctx! { step: step as i64, outcome: outcome },
            ),
            AgentEvent::Replanned { reason, new_steps } => (
                "agent.replanned",
                ctx! { reason: reason, new_steps: new_steps as i64 },
            ),
            AgentEvent::GoalReached { goal, total_steps } => (
                "agent.goal_reached",
                ctx! { goal: goal, total_steps: total_steps as i64 },
            ),
        };
        self.engine.render(&mut self.session, key, &ctx)
    }
}
```

In the agent loop, narrate each step as it lands:

```rust
let mut narrator = AgentNarrator::new()?;

narrator.narrate(AgentEvent::PlanProposed {
    goal: "audit the deployment manifest".into(),
    steps: 4,
})?;
// "Proposed a plan to audit the deployment manifest in four steps."

narrator.narrate(AgentEvent::ToolInvoked {
    tool: "read_file".into(),
    target: "manifest.yaml".into(),
})?;
// "Invoked the read_file tool against manifest.yaml."

narrator.narrate(AgentEvent::StepCompleted {
    step: 1,
    outcome: "manifest parsed cleanly".into(),
})?;
// "Step 1st completed: manifest parsed cleanly."
```

The `Session` carries discourse state across calls — by the third tool invocation the same file gets pronominalized, and by the fifth the connective rotation kicks in.

## Pattern B — `prosaic-tracing` bridge

When the agent already emits `tracing` events, you don't need to wire a narrator into the loop. Drop `prosaic-tracing` into the subscriber stack and templates fire automatically when matching events are recorded.

```toml
[dependencies]
prosaic-core = "0.6"
prosaic-grammar-en = "0.6"
prosaic-tracing = "0.6"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["registry"] }
```

```rust
use prosaic_core::{Engine, Session, Strictness, Variation};
use prosaic_grammar_en::English;
use prosaic_tracing::ProsaicLayer;
use tracing_subscriber::prelude::*;

fn build_narration_layer(out: std::fs::File) -> ProsaicLayer<std::fs::File> {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent) // skip events that have no template
        .variation(Variation::Seeded(0xA6E47));

    engine
        .register_template(
            "agent.tool_invoked",
            "Invoked the {tool} tool against {target}.",
        )
        .unwrap();
    engine
        .register_template(
            "agent.step_completed",
            "Step {step|ordinal} completed: {outcome}.",
        )
        .unwrap();
    engine
        .register_template(
            "agent.replanned",
            "Replanned after {reason}, now {new_steps|words} {new_steps|pluralize:step}.",
        )
        .unwrap();

    ProsaicLayer::new(engine, out)
}

let narration = build_narration_layer(
    std::fs::File::create("agent-narrative.txt").unwrap()
);
let subscriber = tracing_subscriber::registry().with(narration);
tracing::subscriber::set_global_default(subscriber).unwrap();

// Inside the agent loop — events fire from wherever:
tracing::info!(
    name: "tool_invoked",
    target: "agent",
    tool = "read_file",
    target = "manifest.yaml",
);
// → "Invoked the read_file tool against manifest.yaml." appears in agent-narrative.txt

tracing::info!(
    name: "step_completed",
    target: "agent",
    step = 1u64,
    outcome = "manifest parsed cleanly",
);
// → "Step 1st completed: manifest parsed cleanly."
```

Events with no matching template are silently skipped — see [Monitoring Alerts to Prose](monitoring-alerts.md) for the full bridge semantics.

## Why use this instead of an LLM summary layer?

Two properties matter for agent narration that LLM summarization doesn't give you:

1. **Auditability.** The narration is a deterministic function of the events the agent actually emitted. If the prose says "the read_file tool was invoked against manifest.yaml," the agent definitely invoked that tool against that target — there is no path by which the prose could have invented either fact. The PARENT faithfulness scoring in `prosaic-core` proves this at runtime if you opt into the gate.

2. **Regression testing.** Because the narration is deterministic, you can golden-test it. A change to the agent that alters the event sequence will show up as a diff in the rendered narrative; a change to a narration template will show up the same way. CI catches drift in either layer.

A useful split: let the LLM produce the agent's *plan* and *replanning rationale* (creative, open-domain), but route every emitted *event* through Prosaic for the narrative log. The plan is in the agent's freeform tool calls; the audit trail is in Prosaic's prose.

## Multi-agent narration

When several agents share an event stream — Wildmason's Fireside multi-agent rooms are an example — there are two reasonable shapes, and the right one depends on whether you want a unified narrative or one per agent.

**One unified narrative across all agents.** Use a single `ProsaicLayer` and override the key mapper to strip the agent id segment so every agent routes to shared templates. The layer's single `Session` produces one coherent narrative threading every agent's events together:

```rust
let layer = ProsaicLayer::new(engine, output)
    .key_mapper(|meta| {
        // Default key is "{target}.{name}". When target is "agent.<id>",
        // strip the id so events from every agent route to the same templates.
        let target = meta.target();
        let stripped = target.strip_prefix("agent.")
            .and_then(|rest| rest.split_once('.').map(|(_id, tail)| tail))
            .unwrap_or(target);
        format!("{}.{}", stripped, meta.name())
    });
```

**A separate narrative per agent.** A `ProsaicLayer` holds its session internally and isn't built to swap sessions per event, so the cleanest path is Pattern A — one `AgentNarrator` per agent, called directly from each agent's loop with its own `Session`. Each narrator's discourse state (pronoun history, connective rotation, list-style cycle) tracks that one agent's narrative without bleed-through. Sharing templates across narrators is fine — build one engine config helper and call it from each narrator's constructor.

## See also

- [Monitoring Alerts to Prose](monitoring-alerts.md) — the closely-related observability cookbook entry; agent narration uses the same `prosaic-tracing` mechanism.
- [Hallucination by construction](../../hallucination-by-construction.md) — why the prose this pattern produces is auditable.
- [Faithfulness Testing](faithfulness-testing.md) — how to assert PARENT precision == 1.0 in CI for narration templates.
