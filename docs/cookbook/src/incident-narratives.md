# Incident Narratives

A sequence of incident events — detected, impact, mitigation, resolved — is
the ideal input for `DocumentPlan`. The plan organizes events into paragraphs,
orders them by salience, and the engine's discourse state threads pronouns and
connectives across them.

## Registering incident templates

```rust
use prosaic_core::{ctx, Engine, Session, Strictness, Variation};
use prosaic_grammar_en::English;

let mut engine = Engine::new(English::new())
    .strictness(Strictness::Silent)
    .variation(Variation::Fixed);

engine.register_template(
    "incident.detected",
    "An incident was detected on {service} at {timestamp}: {description}",
).unwrap();

engine.register_template(
    "incident.impact",
    "The incident affected {affected_users|quantify} \
     {affected_users|pluralize:user} across \
     {affected_regions|join}{?error_rate}, with error rates \
     reaching {error_rate}{/?}",
).unwrap();

engine.register_template(
    "incident.mitigation",
    "{responder} applied {action}\
     {?duration} within {duration} of detection{/?}",
).unwrap();

engine.register_template(
    "incident.resolved",
    "The incident was resolved at {timestamp}\
     {?root_cause}. Root cause: {root_cause}{/?}\
     {?total_duration}. Total duration: {total_duration}{/?}",
).unwrap();
```

## Building and rendering the narrative

```rust
use prosaic_core::{Context, DocumentPlan, GroupingStrategy};

let events: Vec<(&str, Context)> = vec![
    ("incident.detected", ctx! {
        service: "checkout-service",
        timestamp: "14:23 UTC",
        description: "elevated 5xx error rate exceeding 5% SLO threshold",
    }),
    ("incident.impact", ctx! {
        affected_users: 12000,
        affected_regions: vec![
            "us-east-1".to_string(),
            "eu-west-1".to_string(),
        ],
        error_rate: "8.3%",
    }),
    ("incident.mitigation", ctx! {
        responder: "Alice Chen",
        action: "a connection pool size increase from 50 to 200",
        duration: "6 minutes",
    }),
    ("incident.resolved", ctx! {
        timestamp: "14:41 UTC",
        root_cause: "connection pool exhaustion under sustained Black Friday traffic",
        total_duration: "18 minutes",
    }),
];

let plan = DocumentPlan::from_events_grouped(
    &events,
    &engine,
    GroupingStrategy::ByEntity,
);

let mut session = Session::new();
let narrative = plan.render(&engine, &mut session).unwrap();
println!("{narrative}");
```

Output:

```
An incident was detected on checkout-service at 14:23 UTC: elevated 5xx error
rate exceeding 5% SLO threshold. The incident affected over ten thousand users
across us-east-1 and eu-west-1, with error rates reaching 8.3%. Alice Chen
applied a connection pool size increase from 50 to 200 within 6 minutes of
detection. The incident was resolved at 14:41 UTC. Root cause: connection pool
exhaustion under sustained Black Friday traffic. Total duration: 18 minutes.
```

## `ByEntity` vs `ByAction` on the same events

`ByEntity` groups consecutive events that share an entity name and orders
paragraphs by highest salience. It produces tight, entity-focused narratives
— good for incident reports where the service name is the natural organizer.

`ByAction` groups by rhetorical category: removals lead, then additions, then
modifications. It's better suited to changelogs and release notes.

For incident events none of the keys (`detected`, `impact`, `mitigation`,
`resolved`) map to standard rhetorical categories, so `ByAction` falls them
all into the `Other` bucket and the output is effectively the same as
sequential rendering. For incident reporting, always use `ByEntity`.

## Batch rendering vs `DocumentPlan`

| | `render_batch` | `DocumentPlan` |
|---|---|---|
| Input | flat slice of events | structured paragraphs |
| Grouping | same-entity clause reduction | full paragraph planning |
| Paragraph breaks | none | yes (blank line between paragraphs) |
| Best for | short runs, deploy lifecycles | incident reports, changelogs |

`render_batch` is simpler — pass events, get a single aggregated string. Use
it when you have three to ten related events and a flat output is fine.

`DocumentPlan` is for longer documents where paragraph breaks, salience
ordering, and multi-paragraph discourse coherence matter.
