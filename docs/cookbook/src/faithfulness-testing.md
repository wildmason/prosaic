# Faithfulness Testing

Prosaic includes a PARENT-style reference-free faithfulness scorer. It checks
two things: whether the rendered output introduces words not grounded in the
context or template (precision), and whether negation tokens are preserved
correctly (polarity). Neither check requires a reference translation.

## What the scorer measures

**Precision.** Every content token in the output is checked against a source
set built from context values and template literal tokens. If the output
contains a word that doesn't appear in either source, it's flagged as
unentailed. A template that hallucinates (e.g. invents a value not in the
context) scores below 1.0.

**Polarity.** Negation tokens (`not`, `never`, `no`, `none`, `cannot`,
`won't`, `neither`, `nor`) are counted separately. If the output has a
different count than the source, `polarity_match` is false. This catches the
class of bug where a conditional flips a negation — "was not merged" becomes
"was merged".

Short words (length < 3), stopwords, and pure digits are excluded from content
scoring. They're noise from the perspective of hallucination detection.

## `assert_faithful!` in vocab tests

The `assert_faithful!` macro is the primary testing tool. It panics with a
detailed diagnostic if the output fails either check:

```rust
use prosaic_core::{assert_faithful, ctx, Engine, Session, Strictness, Variation};
use prosaic_grammar_en::English;

#[test]
fn rename_template_is_faithful() {
    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed);

    engine
        .register_template(
            "code.renamed",
            "{old_name} was renamed to {new_name}\
             {?consumer_count}, affecting {consumer_count} \
             {consumer_count|pluralize:consumer}{/?}",
        )
        .unwrap();

    let context = ctx! {
        old_name: "AuthHelper",
        new_name: "AuthService",
        consumer_count: 14,
    };

    let mut session = Session::new();
    let output = engine
        .render(&mut session, "code.renamed", &context)
        .unwrap();

    // Template literals provide: "was", "renamed", "to", "affecting", "consumer"
    // Context provides: "AuthHelper", "AuthService", "14"
    let lits: &[&str] = &["was renamed to", ", affecting", "consumer"];
    assert_faithful!(output, context, lits, &English::new());
}
```

If the test fails, the panic message shows which tokens are unentailed and
what the polarity drift is — enough to diagnose the template without
re-running the engine manually.

## Runtime faithfulness gate

For production systems, configure the engine to reject unfaithful output at
render time:

```rust
let engine = Engine::new(English::new())
    .with_faithfulness_gate(1.0); // reject any output with precision < 1.0
```

Any render that produces output below the threshold returns a
`ProsaicError::FaithfulnessViolation` instead of the string. Handle it like
any other render error:

```rust
match engine.render(&mut session, "key", &ctx) {
    Ok(prose) => emit(prose),
    Err(ProsaicError::FaithfulnessViolation { .. }) => {
        // fall back to raw data or skip this event
    }
    Err(e) => return Err(e),
}
```

A threshold of `1.0` is strict — every content token must be grounded. A
threshold of `0.85` tolerates a small fraction of unentailed tokens, which is
useful when templates legitimately introduce hedging language (`likely`,
`approximately`, `roughly`) that won't appear in the context.

## Scoring directly

Call `score_faithfulness` directly for custom threshold logic or diagnostic
reporting:

```rust
use prosaic_core::score_faithfulness;
use prosaic_grammar_en::English;

let score = score_faithfulness(
    &output,
    &context,
    &["was renamed to", ", affecting"],
    &English::new(),
);

println!("precision: {:.3}", score.precision);
println!("polarity_match: {}", score.polarity_match);
if !score.unentailed.is_empty() {
    println!("unentailed tokens: {:?}", score.unentailed);
}

if score.passes(0.9) {
    emit(output);
}
```

`FaithfulnessScore::is_faithful()` requires both `precision == 1.0` and
`polarity_match`. `FaithfulnessScore::passes(threshold)` relaxes the
precision check while keeping the polarity gate strict.

## Known false positives

**Morphological edge cases.** The scorer uses singularization for tolerance
(`"consumers"` matches `"consumer"`), but it doesn't handle all inflections.
Irregular plurals (`mice`/`mouse`) or compound nouns may score as unentailed
even when they're legitimately derived from context.

**Template literals that look like content.** If your template contains a
domain-specific word that's also a content token — e.g., `"the API gateway"` —
it contributes to the source set. But if you have a template that says
`"the payment gateway experienced"` and the context doesn't contain
`"payment"`, that token must appear in your `template_literals` slice, not
just in the template source. The `literal_tokens()` method on `Template` can
build this slice automatically:

```rust
let template = engine.get_template("code.renamed").unwrap();
let lits: Vec<&str> = template.literal_tokens();
let score = score_faithfulness(&output, &context, &lits, &English::new());
```

**Partials.** `literal_tokens()` does not recurse into partial expansions.
If your template includes `{>some_partial}` and the partial introduces content
words, those words won't be in the source set and may be flagged as unentailed.
Pre-expand the partial or call `score_faithfulness` with the partial's literal
tokens included in the slice.

## Referential success testing

PARENT precision catches *token-level fabrication* — content words in the
output that don't trace to the input. It does not catch *referential
ambiguity* — descriptions that are token-faithful but don't actually
identify which entity they're about.

> "The class was renamed to AccountService."

is precision = 1.0 if "class", "renamed", and "AccountService" all entail to
source. But if two classes were both touched in the same render batch, the
description fails to pick out which one. A reader can't recover the target.

Chen, Same, & van Deemter (*Intrinsic Task-based Evaluation for Referring
Expression Generation*, arXiv:2402.07432, 2024) propose a *referential
success* protocol as a second evaluation axis: a participant is shown the
generated referring expression and asked to identify the target from a
candidate set. The expression is "successful" only when the participant
picks the right entity.

Adapted to deterministic test infrastructure, this becomes an algorithmic
disambiguation check: given a rendered referring expression and a candidate
set of `EntityDescriptor`s, does the expression's distinguishing attributes
uniquely match exactly one candidate?

```rust
use prosaic_core::{Engine, Session, Strictness, EntityDescriptor};
use prosaic_grammar_en::English;

#[test]
fn full_form_uniquely_identifies_target_among_distractors() {
    let mut engine = Engine::new(English::new()).strictness(Strictness::Strict);
    engine.register_template(
        "code.renamed",
        "{old_name|refer} was renamed to {new_name}.",
    ).unwrap();

    // The target and a same-type distractor.
    let target = EntityDescriptor::new("AuthHelper", "class")
        .with_attribute("layer", "infra");
    let distractor = EntityDescriptor::new("UserHelper", "class")
        .with_attribute("layer", "domain");

    let mut session = Session::new();
    engine.register_entity(target.clone());
    engine.register_entity(distractor.clone());

    let ctx = ctx! {
        old_name: "AuthHelper",
        new_name: "AuthService",
    };
    let output = engine.render(&mut session, "code.renamed", &ctx).unwrap();

    // Referential success: the rendered expression must contain enough
    // disambiguating signal to pick out the target. With two same-type
    // entities in the discourse, the bare "the class" is not enough — the
    // expression should include the discriminating attribute (e.g. "the
    // infra class AuthHelper").
    assert!(
        output.contains("infra") || output.contains("AuthHelper"),
        "rendered expression failed to disambiguate target: {output}"
    );
}
```

The check above is loose — it's a contains-based smoke test. A stricter
version asserts that *the rendered expression's distinguishing tokens match
the target descriptor and not the distractor*:

```rust
/// All tokens — name and attribute values — that distinguish an entity.
fn distinguishing_tokens(e: &EntityDescriptor) -> Vec<String> {
    let mut out = vec![e.name.clone()];
    out.extend(e.attributes.iter().map(|(_k, v)| v.clone()));
    out
}

fn referentially_unique(
    output: &str,
    target: &EntityDescriptor,
    distractors: &[EntityDescriptor],
) -> bool {
    let target_signal: Vec<String> = distinguishing_tokens(target)
        .into_iter()
        .filter(|t| output.contains(t.as_str()))
        .collect();
    if target_signal.is_empty() {
        return false; // no target-specific signal in the output
    }

    for d in distractors {
        let d_signal: Vec<String> = distinguishing_tokens(d)
            .into_iter()
            .filter(|t| output.contains(t.as_str()))
            .collect();
        if d_signal.len() >= target_signal.len() {
            return false; // distractor matches at least as well — ambiguous
        }
    }
    true
}
```

Call it from a test:

```rust
#[test]
fn renamed_template_is_referentially_unique() {
    // ... build target, distractors, render output as above ...
    assert!(
        referentially_unique(&output, &target, &distractors),
        "render is referentially ambiguous"
    );
}
```

### When to use referential-success tests vs PARENT precision

- **PARENT precision** catches a template that emits content not grounded
  in the input. Run it on every vocab template. If precision drops below
  1.0, the template is leaking — fix the template.
- **Referential-success** catches a template that emits *correctly grounded*
  content that nonetheless fails to disambiguate among entities present in
  the discourse. Run it on templates that mention named entities when more
  than one entity of the same type is plausibly in scope. If the test
  fails, the issue is usually that the template asks for a `Pronoun` /
  `ShortName` form when an ambiguity demanded `Full`. The fix is in the
  REG selection logic (`reg.rs`) or in the registration order, not in the
  template.

PARENT and referential-success are complementary: together they cover
"the description is true of the target" *and* "the description picks out
the target uniquely." Either alone leaves a gap.

### Limitation

This methodology is deterministic and cheap, but it tests *attribute
disambiguation*, not *human referential success* in the strict
psycholinguistic sense Chen et al. propose. A full reproduction of their
protocol would require human evaluators or — at minimum — a deterministic
classifier that simulates a human picking among candidates. The
attribute-disambiguation check is the floor; the human protocol is the
ceiling. For CI and template authoring, the floor is enough.
