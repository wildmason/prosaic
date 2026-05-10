# Related work and design heritage

Prosaic's design is not novel. Almost every mechanism in the engine — the discourse state, the referring-expression generator, the rhetorical structure layer, the aggregation/ellipsis module, the faithfulness scorer — implements an idea from the natural-language-generation research literature, often a decades-old one. What is novel is the *combination*: assembling those mechanisms into a single deterministic, no-LLM Rust crate at production quality, then layering a small set of contemporary techniques (PARENT-style faithfulness scoring, Self-Refine-style retrospective refinement, persona-as-configuration) on top of the classical foundation.

This document is the citation map. It exists for two reasons: to credit the prior work the engine is built on, and to make Prosaic's positioning relative to current NLG research legible to anyone evaluating it.

## The field is converging on LLMs. Prosaic is deliberately not.

Van Miltenburg & Lin's 2025 survey *Natural Language Generation* (arXiv:2503.16728, Encyclopedia of Language & Linguistics) opens with the field-framing observation: "with the rise of Large Language Models (LLMs), different subfields of Natural Language Processing have converged on similar methodologies for the production of natural language and the evaluation of automatically generated text." That convergence is real and largely productive — for open-domain text generation, an LLM is in most cases the right tool.

Reiter's 2024 chapter *Rule-Based NLG* (Chapter 2 of *Natural Language Generation*, Springer, DOI 10.1007/978-3-031-68582-8_2) is the same field's authoritative voice arguing the other half of the picture: in the post-LLM era, rule-based NLG is not a legacy choice but the correct architecture for applications that require auditability and complete factual fidelity. Reiter — who literally co-wrote the foundational textbook on the subject — making this argument in 2024 is the strongest external endorsement Prosaic's design thesis is likely to receive. Prosaic is the operational instantiation of the position Reiter defends in print.

Prosaic treats deterministic, rule-based NLG not as a stepping stone superseded by neural models, but as the correct architecture for a specific class of problem: structured-data narration where every content token must be defensible against the input, every render must be reproducible across runs, and the cost surface must be a constant, not a per-token API charge. The categories where that trade is the right one — changelogs, regulatory text, audit narration, observability prose, agent narration logs — are growing as more systems need machine-generated prose that can survive code review and legal review. Prosaic exists for those.

The remainder of this document maps the engine onto its sources.

## Classical foundations

The engine's structural skeleton is Reiter & Dale's NLG pipeline. The current crate layout closes over it almost one-to-one.

| Stage | Source | Where it lives in Prosaic |
|---|---|---|
| Content planning | Reiter & Dale, *Building Natural Language Generation Systems* (2000) | `DocumentPlan` in `prosaic-core/src/document.rs` — grouping, ordering, salience-tier classification |
| Microplanning (sentence planning) | same | `Engine::render_batch_with_relations`, `RST` relations in `rst.rs`, aggregation in the choose-best scorer |
| Realisation (surface generation) | same | `Engine::render`, the template parser in `template.rs`, the per-language `Language` impls |

The decision to keep these stages separable, rather than collapsing them into a single end-to-end function, is the reason discourse state and faithfulness scoring can sit between them as orthogonal layers.

## Specific component heritage

Each row below is a Prosaic mechanism and the academic or industrial work it implements.

### Referring-expression generation (REG)

- **Dale & Reiter, "Computational Interpretations of the Gricean Maxims in the Generation of Referring Expressions" (1995).** The Incremental Algorithm — choose attributes by discriminating power, stop when the description uniquely identifies — is the default backend in `prosaic-core/src/reg.rs`.
- **Krahmer, van Erk, & Verleg, "Graph-based Generation of Referring Expressions" (2003).** The opt-in second backend with relations, gated behind the `reg` feature, implements Krahmer's graph formulation as an alternative when the input naturally carries entity relationships.

### Discourse coherence

- **Grosz, Joshi, & Weinstein, "Centering: A Framework for Modeling the Local Coherence of Discourse" (1995).** Cb (backward-looking center) tracking, Cf (forward-looking center) ranking, and Centering Rule 1 (pronoun gating) are implemented in `prosaic-core/src/discourse.rs`. The `Session` carries the centering state across renders inside a logical document.
- **Mann & Thompson, "Rhetorical Structure Theory: Toward a Functional Theory of Text Organization" (1988).** RST relations (contrast, sequence, elaboration, cause) drive connective selection in `discourse.rs::select_connective` and structural organization in `DocumentPlan`. Connective recency windows and family budgets are an engineering extension of the RST tradition, not part of the original theory.

### Aggregation and ellipsis

- **Reape & Mellish, "Just What is Aggregation Anyway?" (1999) and Gatt & Reiter's SimpleNLG (2009).** Aggregation patterns — same-entity-different-actions, same-action-different-entities — are the source of `Engine::render_batch`'s merging rules. SimpleNLG was considered as a phrasal realizer dependency and rejected (see *Approaches considered and rejected* below); the patterns survived.
- **Harbusch & Kempen's ELLEIPO line of work (2009).** Forward conjunction reduction — the safe subset of gapping — is implemented in the aggregation pass. Full ELLEIPO with arbitrary gapping remains a Tier-2 deferred item per the engineering roadmap.

### Faithfulness

- **Dhingra et al., "Handling Divergent Reference Texts when Evaluating Table-to-Text Generation" (PARENT, 2019).** The reference-free precision score in `prosaic-core/src/faithfulness.rs` is a PARENT-style adaptation: the union of Context tokens plus template literals is the source set, and entailment is computed against it with bidirectional singularization. The polarity gate (`POLARITY_TOKENS` multiset comparison) is a Prosaic extension specifically targeting negation drift, which generic PARENT precision misses.

### Industrial influences

- **RosaeNLG** (Renaud Pawlak, et al., 2017–). The `choosebest` weighted-history selection, the four-style list-formatting palette (`including X among others` / `such as X` / em-dash / bracketed), and the explicit-pipe template syntax are direct echoes. The migration cookbook entry at `docs/cookbook/src/migrating-from-rosaenlg.md` is the formal acknowledgment.
- **SimpleNLG** (Gatt & Reiter, 2009; current Java + multilingual ports). Considered as a phrasal-realizer dependency for English; rejected as a runtime dependency in favor of native Rust grammar packs that compose with `no_std + alloc`. The aggregation patterns and the abstract notion of separating microplanning from realization survived.

## Modern NLG research and how it maps onto Prosaic

The contemporary NLG literature is overwhelmingly LLM-centric. Most of it does not transfer to Prosaic mechanically. A small number of techniques port cleanly when the *concept* is separated from the LLM-specific *mechanism*.

### Adapted into Prosaic

- **Huang et al., *A Survey on Hallucination in Large Language Models* (arXiv:2311.05232, 2023, rev. 2024).** The taxonomy (factuality vs faithfulness; entity error, relation error, factual fabrication, instruction inconsistency, context inconsistency, logical inconsistency; data/training/inference contributing factors) is the framework `docs/hallucination-by-construction.md` uses to defend Prosaic's "no hallucination" claim category by category.
- **Madaan et al., *Self-Refine: Iterative Refinement with Self-Feedback* (arXiv:2303.17651, NeurIPS 2023).** The generate → critique → refine loop, with the LLM critic replaced by deterministic diagnosers and the LLM refiner replaced by re-rendering under additive constraints, is the design basis for the retrospective-pass spec at `docs/superpowers/specs/2026-05-09-self-refine-retro-pass-design.md`.
- **Xu et al., *Personalized Generation In Large Model Era: A Survey* (arXiv:2503.02614, 2025).** The "long-term persona memory" framing — durable per-consumer voice that persists across interactions — is the design basis for the `StyleProfile` spec at `docs/superpowers/specs/2026-05-09-style-profile-design.md`. Mem0-style neural memory does not port; declarative configuration does.
- **Plaat et al., *Agentic Large Language Models, a survey* (arXiv:2503.23037, 2025).** The "NLG as tool, not output" framing is the design basis for the agent-narration cookbook at `docs/cookbook/src/agent-narration.md`. Agentic systems can call Prosaic for deterministic, auditable narration of their own structured event streams.

### Surveyed but not adapted

- **van Miltenburg & Lin, *Natural Language Generation* (arXiv:2503.16728, 2025).** Cited as the field-positioning anchor; no specific technique to import — it is an encyclopedia overview.
- **Liu et al., *G-Eval: NLG Evaluation using GPT-4 with Better Human Alignment* (EMNLP 2023, arXiv:2303.16634).** LLM-as-judge for NLG quality scoring. Considered as the basis for a dev-time evaluation harness and **rejected**: bringing an LLM dependency into Prosaic — runtime, dev-time, or anywhere in the stack — undercuts the design thesis the rest of the engine is built on. Quality validation stays in the deterministic toolchain (PARENT precision gate, golden corpora, document-scope diagnosers from the Self-Refine retro-pass spec).
- **Bai et al., *LongWriter: Unleashing 10,000+ Word Generation from Long Context LLMs* (arXiv:2408.07055, 2024).** Solves an LLM-only bottleneck (training-data length distribution caps coherent output at ~2,000 words). Prosaic's `DocumentPlan` + RST + paragraph reset already handles arbitrary length; the only adjacent value is using LongWriter as a future *benchmark target* for long-form narration parity.
- **Kumar, Dao, & May, *Speculative Speculative Decoding* (arXiv:2603.03251, 2026).** Pure LLM inference-speed optimization via parallelized draft/verify. Prosaic has no autoregressive token loop; nothing to parallelize at the decoding layer.
- **Anthropic, *Natural Language Autoencoders* (2026).** LLM interpretability technique that round-trips activations through human-readable text. Not applicable: Prosaic has no activations to verbalize.

## Approaches considered and rejected

This section records design choices Prosaic actively turned down, with the reasons. Each was a real branch point during development.

- **Grammatical Framework (GF) as the realization backend.** Considered for its strong type-theoretic guarantees and multilingual coverage. Rejected: the dependency footprint and the GF runtime contract are a poor fit for `no_std + alloc` and for a single-binary `cargo install` story. Native Rust grammar packs are heavier per-language but cleaner end-to-end.
- **MessageFormat 2.0 as the template surface.** Considered for cross-platform pluralization and gender semantics. Rejected as a wholesale template format because the surface syntax is too verbose for prose-density templates; the *plural-category* semantics were cherry-picked into the `pluralize` and `proportion` pipes instead.
- **SimpleNLG as the phrasal realizer.** Considered as a way to get English morphology and basic syntax for free. Rejected as a runtime dependency (Java/JVM coupling, port overhead) but kept as a design influence for aggregation patterns.
- **SDRT over RST.** Segmented Discourse Representation Theory was considered as a more expressive alternative to Mann & Thompson's RST. Rejected: SDRT's added expressivity does not pay rent in the data-to-text use case Prosaic optimizes for, and RST has a wider implementation literature for the connective-family decisions Prosaic actually makes.
- **Any LLM dependency in Prosaic, runtime or dev-time.** Two specific instances were considered and turned down: a Tier-3 `prosaic-polish-llm` hybrid paraphrase pass (Paraphraser+Verifier sandwich) and a G-Eval-based dev-time evaluation harness (LLM-as-judge over rendered prose for CI quality gates). Both rejected for the same reason: the design thesis of the engine is that prose can be defended against the input by structural means alone, and adding an LLM dependency anywhere in the stack — feature-flagged, gated, dev-only, or otherwise — would dilute that thesis to the point of meaninglessness. If a paraphrase capability or an external-model eval suite ever ships, it ships as a separate companion crate that consumers can opt into themselves; it is not part of Prosaic.
- **Reference-based evaluation metrics (BLEU, ROUGE).** Used internally for spot checks in the early development phase, never adopted as a quality signal. Reference-based metrics require gold-standard outputs that don't exist for Prosaic's structured-data → prose use case; PARENT-style reference-free scoring is the right primitive.

## What this means for Prosaic's positioning

Prosaic occupies the **deterministic, neuro-symbolic, post-LLM-convergence** corner of the NLG design space. "Post-LLM-convergence" is the load-bearing phrase: this is not a pre-LLM library that hasn't caught up, it is a deliberately divergent design that takes the LLM-era literature seriously and rejects the convergence on its specific use case. The defense of that position is empirical (the rendered prose is competitive with LLM output for structured-data narration) and structural (the failure modes the LLM literature catalogs are absent or contained by construction; see `docs/hallucination-by-construction.md`).

For consumers evaluating Prosaic against an LLM-based narration layer, the relevant questions are:

1. Does your prose need to be defensible token by token against the input? If yes, this is the right architecture.
2. Does your cost surface need to be a one-time integration cost rather than a per-token API charge? If yes, this is the right architecture.
3. Does your prose need to handle topics nobody anticipated at template-authoring time? If yes, this is the wrong architecture and an LLM is the right tool.

These are the questions to put on the slide. The rest of this document is the receipts.

## References used in this document

The list below is the set of works actually cited above; it is not a complete NLG bibliography.

- Bai, Y., Zhang, J., Lv, X., Zheng, L., Zhu, S., Hou, L., Dong, Y., Tang, J., Li, J. *LongWriter: Unleashing 10,000+ Word Generation from Long Context LLMs.* arXiv:2408.07055 (2024).
- Dale, R., Reiter, E. *Computational Interpretations of the Gricean Maxims in the Generation of Referring Expressions.* Cognitive Science (1995).
- Dhingra, B., Faruqui, M., Parikh, A., Chang, M-W., Das, D., Cohen, W. *Handling Divergent Reference Texts when Evaluating Table-to-Text Generation* (PARENT). ACL 2019.
- Gatt, A., Reiter, E. *SimpleNLG: A Realisation Engine for Practical Applications.* ENLG 2009.
- Grosz, B., Joshi, A., Weinstein, S. *Centering: A Framework for Modeling the Local Coherence of Discourse.* Computational Linguistics (1995).
- Harbusch, K., Kempen, G. *Generating Coordinate Ellipsis* (ELLEIPO line of work).
- Huang, L., Yu, W., Ma, W., Zhong, W., Feng, Z., Wang, H., Chen, Q., Peng, W., Feng, X., Qin, B., Liu, T. *A Survey on Hallucination in Large Language Models: Principles, Taxonomy, Challenges, and Open Questions.* arXiv:2311.05232 (2023, rev. 2024).
- Krahmer, E., van Erk, S., Verleg, A. *Graph-based Generation of Referring Expressions.* Computational Linguistics (2003).
- Kumar, T., Dao, T., May, A. *Speculative Speculative Decoding.* arXiv:2603.03251 (2026).
- Liu, Y., Iter, D., Xu, Y., Wang, S., Xu, R., Zhu, C. *G-Eval: NLG Evaluation using GPT-4 with Better Human Alignment.* EMNLP 2023, arXiv:2303.16634.
- Madaan, A., Tandon, N., Gupta, P., Hallinan, S., Gao, L., Wiegreffe, S., Alon, U., Dziri, N., Prabhumoye, S., Yang, Y., Gupta, S., Majumder, B. P., Hermann, K., Welleck, S., Yazdanbakhsh, A., Clark, P. *Self-Refine: Iterative Refinement with Self-Feedback.* arXiv:2303.17651, NeurIPS 2023.
- Mann, W., Thompson, S. *Rhetorical Structure Theory: Toward a Functional Theory of Text Organization.* Text (1988).
- Plaat, A., van Duijn, M., van Stein, N., Preuss, M., van der Putten, P., Batenburg, K. J. *Agentic Large Language Models, a survey.* arXiv:2503.23037 (2025, rev. Nov 2025).
- Reape, M., Mellish, C. *Just What is Aggregation Anyway?* (1999).
- Reiter, E. *Rule-Based NLG.* Chapter 2 in *Natural Language Generation*, Springer (2024). DOI 10.1007/978-3-031-68582-8_2.
- Reiter, E., Dale, R. *Building Natural Language Generation Systems.* Cambridge University Press (2000).
- van Miltenburg, E., Lin, C. *Natural Language Generation.* arXiv:2503.16728 (2025), Encyclopedia of Language & Linguistics.
- Xu, Y., Zhang, J., Salemi, A., Hu, X., Wang, W., Feng, F., Zamani, H., He, X., Chua, T-S. *Personalized Generation In Large Model Era: A Survey.* arXiv:2503.02614 (2025, rev. May 2025).
