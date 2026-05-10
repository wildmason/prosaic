# Hallucination by construction

Prosaic does not hallucinate. This document explains what that means precisely — category by category, against the canonical taxonomy of LLM hallucinations from Huang et al. (2023, rev. 2024) — and where the guarantees end.

## Why this document exists

"Prosaic doesn't hallucinate" sounds like marketing. In an LLM-saturated industry, the claim has to be defended in the language the field actually uses, not in hand-waved generalities. Huang et al.'s *A Survey on Hallucination in Large Language Models: Principles, Taxonomy, Challenges, and Open Questions* (arXiv:2311.05232) is the most widely cited current taxonomy. The rest of this document maps each leaf of that taxonomy onto a Prosaic structural property and shows why the failure mode is either impossible by design or detected by an existing runtime gate.

The point is not to argue that Prosaic produces *better* prose than an LLM. The point is that a different class of system has a different failure surface, and Prosaic's surface excludes the failure modes the taxonomy was built to describe.

## The taxonomy in one paragraph

Huang et al. split hallucination into two top-level categories. **Factuality hallucination** covers divergence from real-world facts — splitting further into *factual contradiction* (entity errors, relation errors) and *factual fabrication* (unverifiable claims, overclaims). **Faithfulness hallucination** covers divergence from the user's input or internal consistency — splitting into *instruction inconsistency*, *context inconsistency*, and *logical inconsistency*. Orthogonally, the survey identifies three families of contributing factors: data-related, training-related, and inference-related.

## Category-by-category mapping

### 1. Factuality hallucination

#### 1.1 Factual contradiction — entity error

> "Generated text contains erroneous entities."
> — Huang et al.

**Prosaic property.** The renderer has no entity store of its own. Every entity name reaching the output came from a `Value::String` or `Value::Entity` inserted at a named slot in the caller's `Context` (`prosaic-core/src/context.rs:19-33`). Templates substitute by slot key; they cannot synthesize a name. Pipes that transform entity references — `refer`, `possessive`, `syn` — are pure functions of the value present in the slot plus the discourse state, not lookups into an external knowledge base.

**Runtime check.** PARENT-style faithfulness scoring treats the union of `Context` token tokens plus template literals as the source set. Any content token in the output that fails entailment is reported in `FaithfulnessScore::unentailed` and reduces `precision` below 1.0 (`prosaic-core/src/faithfulness.rs:65-205`). With the runtime gate enabled (`FaithfulnessRejection` in `error.rs:32-36`), an entity that "leaks in" from anywhere other than the input is rejected and the session state rolls back.

#### 1.2 Factual contradiction — relation error

> "Generated text contains wrong relations between entities."

**Prosaic property.** Relations between entities are encoded in template literals — fixed text the developer authored. `"{old_name|refer} was renamed to {new_name}"` literally states the relation; the engine substitutes the two slots and no decision about which verb pairs with which noun is made at render time. Relation correctness is therefore a property of the authored template, not an emergent property of generation.

**Type-aware authoring guard.** `Engine::register_template_with_schema<T>` (`engine.rs:2112`) cross-checks every slot's pipe-inferred type against `T::PROSAIC_SCHEMA`. A template that types `{count|pluralize:item}` requires `count: Number`; mismatch with the schema is rejected at registration with a `TemplateParseError`. This catches a class of relation-shaped errors — "this slot was declared as a list but the template treats it as a number" — at load time, before any user-facing render.

#### 1.3 Factual fabrication — unverifiability

> "Statements entirely non-existent or cannot be verified using available sources."

**Prosaic property.** There are no "available sources" beyond the `Context` and the template literals. Both are explicit, finite, and inspectable. Every content token in the output must be entailed by that union or it is, by definition of the PARENT precision scorer, unverifiable — and is reported as such.

**The gate.** `FaithfulnessScore::is_faithful()` is `true` iff `precision == 1.0` and the polarity multiset matches between source and output (`faithfulness.rs:92-98`). Strict templates can use the `assert_faithful!` macro in tests (`faithfulness.rs:283-313`); production callers can opt into the runtime `FaithfulnessRejection` path.

#### 1.4 Factual fabrication — overclaim

> "Claims lacking universal validity due to subjective biases."

**Prosaic property.** Quantification is governed by explicit pipes — `quantify`, `proportion`, `hedge` — each with documented input-to-output mappings. `{conf|hedge}` over a context value of `conf=30` produces `"possibly"` deterministically; `{n|quantify}` over `n=300` produces `"hundreds of"`; `{2|proportion:2}` produces `"both"`. There is no aggregated training corpus that could nudge the system toward sweeping claims. The author chooses the hedge layer; the engine does not amplify confidence.

### 2. Faithfulness hallucination

#### 2.1 Instruction inconsistency

> "LLM's outputs deviate from a user's directive."

**Prosaic property.** There is no instruction-following step. The "instruction" is the registered template plus its pipe chain — both authored, both executed by deterministic substitution. The engine cannot disregard the template because the template *is* the program. `Strictness::Strict` further hardens this: a missing slot returns `ProsaicError::MissingSlot` rather than improvising filler text (`engine.rs:704`); an unknown pipe returns `ProsaicError::InvalidPipe` (`engine.rs:788`). The system never silently widens the directive.

#### 2.2 Context inconsistency

> "LLM's output unfaithful with user's provided contextual information."

**Prosaic property.** This is the category PARENT was built to address. The faithfulness scorer's source set is exactly the caller's `Context` plus the template's literal tokens, with bidirectional singularization for morphological tolerance (`faithfulness.rs:174-205`). Output content tokens that cannot be entailed by that set are flagged as unentailed and counted against precision.

**Polarity gate.** Negation drift is its own failure mode and is treated separately. The polarity tokens — `not`, `never`, `no`, `none`, `cannot`, `won't`, `neither`, `nor` — are excluded from content scoring and instead checked as a multiset: their counts in the source must equal their counts in the output (`faithfulness.rs:141-163`). A render that turns "X was not deleted" into "X was deleted" fails the polarity gate even if every content token is entailed. `is_faithful()` requires both gates.

#### 2.3 Logical inconsistency

> "LLM outputs exhibit internal logical contradictions, often in reasoning tasks."

**Prosaic property.** Prosaic does not perform reasoning. It substitutes structured data into authored prose. The engine cannot introduce a contradiction the template doesn't already contain: if every slot is filled from a single `Context` and the template never re-reads the same slot under conflicting framings, the output is consistent by construction. (See *Boundaries* below for what this guarantee does and does not cover.)

## Contributing factors

Huang et al.'s contributing-factors axis is orthogonal to the symptom taxonomy — it asks where in the LLM lifecycle the failure originated. The mapping to Prosaic is short because most of these factors require pipeline stages Prosaic does not have.

| Family | Examples from Huang | Prosaic exposure |
|---|---|---|
| **Data-related** | imitative falsehood, societal bias, knowledge-boundary limits, copyright-sensitive training data | None. Prosaic has no training corpus. Knowledge in Prosaic is the set of registered templates and vocabulary modules; all of it is explicitly authored, version-controlled, and inspectable. |
| **Training-related** | causal-modeling drift, exposure bias, RLHF sycophancy, inability to reject | None. Prosaic has no training process. The closest analog is the template authoring step, which is human review, not a gradient update. |
| **Inference-related** | imperfect decoding, softmax bottleneck, over-confidence, reasoning failure | None. Prosaic does no probabilistic decoding. A render is a deterministic function of `(template, context, session, language pack)`. There is no temperature, no sampling, no beam — and therefore none of the failure modes those mechanisms generate. |

## Boundaries — what the guarantee does *not* cover

Strong claims earn precise boundaries. Prosaic's hallucination guarantee covers the categories above; it does not cover the following.

1. **Garbage in, faithfully out.** If the caller inserts a wrong fact — `Value::String("AccountService")` for a class that was actually renamed to `BillingService` — Prosaic will faithfully render the wrong fact. This is a property of the input, not a hallucination by the renderer. Faithfulness scoring confirms the output is entailed by the input; it cannot confirm the input is true.

2. **Author-level template errors.** A template that says `"{old_name} is the same as {new_name}"` will produce that statement regardless of whether the relation holds in the world. Logical and factual correctness of templates is a code-review property, like correctness of any other authored function. The engine guarantees that what was written is what is produced; it does not guarantee what was written is true.

3. **Semantic-level fabrication composed of source-entailed tokens.** PARENT precision is a content-token gate. A pathological template could in principle reorder source tokens in a way that changes meaning while keeping every token entailed. Prosaic's runtime defenses against this are limited to the polarity gate; deeper semantic faithfulness is the template author's responsibility.

4. **Polarity gate is English-only.** The current `POLARITY_TOKENS` set (`faithfulness.rs:51-53`) covers English negations. Spanish, German, and other grammars do not yet have a `Language::polarity_tokens()` extension point. Cross-linguistic polarity drift is a documented limitation, not a guarantee.

5. **Partials are opaque to the scorer.** Template literals retrieved via `Template::literal_tokens()` do not expand `{>partial_name}` references. Templates that rely heavily on partials for prose may score lower precision than the actual entailment warrants. Callers can pre-expand partials or disable the gate for those templates (`faithfulness.rs:27-32`).

6. **Numeric-only content is excluded from precision.** Pure-digit tokens are not counted as content tokens (`faithfulness.rs:239-245`), since the polarity-style tooling for numerics would need its own design. A render that swaps `5` for `7` will not be caught by the scorer; it will be caught by anyone reading the output.

These boundaries are documented in source comments and in this file. Future work — a `Language::polarity_tokens()` trait method, a partial-expansion mode for the scorer, a numeric-fidelity gate — is tracked in the realism roadmap rather than promised here.

## What this means in practice

Prosaic occupies a different point on the cost/correctness curve than an LLM. An LLM trades determinism for fluency: it can produce prose about topics nobody anticipated, at the cost of being unable to prove any specific token's provenance. Prosaic trades open-domain coverage for provability: every content token has a source, every relation is authored, every quantifier is a pure function. For applications where the output needs to defend itself — changelogs, audit narration, regulatory text, status updates that get re-read by lawyers — that trade is the right one. For applications where the output is judged on novelty, it isn't.

Treat this document as the answer to "but how do you know it doesn't hallucinate?" Cite the file:line references when you need to show your work.

## References

- Huang, L., Yu, W., Ma, W., Zhong, W., Feng, Z., Wang, H., Chen, Q., Peng, W., Feng, X., Qin, B., Liu, T. *A Survey on Hallucination in Large Language Models: Principles, Taxonomy, Challenges, and Open Questions.* arXiv:2311.05232 (Nov 2023, rev. Nov 2024).
- `prosaic-core/src/faithfulness.rs` — PARENT-style precision and polarity gates.
- `prosaic-core/src/context.rs` — `Context` and `Value` definitions; the bounded input surface.
- `prosaic-core/src/engine.rs` — `Strictness`, `register_template_with_schema`, the runtime gates.
- `prosaic-core/src/error.rs` — `ProsaicError::MissingSlot`, `InvalidPipe`, `FaithfulnessRejection`.
- `docs/plans/parent-faithfulness.md` — design rationale for the PARENT scorer.
