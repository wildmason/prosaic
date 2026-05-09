# Prosaic realism before/after samples

Evidence commands used for the rendered samples:

- `cargo run -p prosaic-core --example list_style_continuity_demo`
- `cargo run -p prosaic-core --example sentence_rhythm_demo`
- `connective_variety_uses_longer_recency_window_in_rendered_prose` in `prosaic-core/tests/integration.rs`
- `service_shape_run_does_not_alternate_similarity_connectives` in `prosaic-core/tests/integration.rs` (regression for the `Similarly,/Likewise,` alternation Matt flagged)
- `connective_family_budget_drops_to_null_when_pool_saturates` and `similarity_family_budget_breaks_service_shape_alternation` in `prosaic-core/src/discourse.rs` unit tests

## 10 deterministic prose pairs

1. List-style continuity, paragraph 1

Before: Touched including Alpha and Beta among others.

After: Touched including Alpha and Beta among others.

Why it matters: The first paragraph remains stable; the fix starts preserving style state for following paragraphs instead of changing the initial rendering.

2. List-style continuity, paragraph 2

Before: Touched including Echo and Foxtrot among others.

After: Touched such as Echo and Foxtrot.

Why it matters: A multi-paragraph document no longer reuses the same list phrasing at every break.

3. List-style continuity, paragraph 3

Before: Touched including India and Juliet among others.

After: Touched — notably India and Juliet, plus 2 more.

Why it matters: The paragraph-scoped reset keeps natural style rotation alive while still clearing paragraph-local reference state.

4. List-style continuity, paragraph 4

Before: Touched including Mike and November among others.

After: Touched [Mike, November, and 2 more].

Why it matters: Long documents get visible list phrasing variety instead of one repeated opener.

5. Connective variety, repeated contrast relation

Before: Meanwhile, the class Delta was deleted.

After: However, the class Delta was deleted.

Why it matters: The longer connective recency window avoids replaying "Meanwhile," when a second contrast appears shortly after the first.

6. Connective variety, repeated same-entity transition

Before: Additionally, the class Delta was modified.

After: Furthermore, the class Delta was modified.

Why it matters: Same-entity follow-up sentences now choose a fresh transition cue instead of cycling straight back to the first connective.

7. Sentence rhythm, service batch

Before: The service BillingService was touched and revalidated against the current schema. Similarly, the service PaymentGateway was touched. Likewise, the service RefundProcessor was touched and revalidated against the current schema. Similarly, the service ReceiptDispatcher was touched. Likewise, the service InvoiceLedger was touched and revalidated against the current schema. Similarly, the service AuditTrail was touched.

After: The service BillingService was touched and revalidated against the current schema. Similarly, the service PaymentGateway was touched. Likewise, the service RefundProcessor was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The service ReceiptDispatcher was touched. The service InvoiceLedger was touched and revalidated against the current schema. The service AuditTrail was touched.

Why it matters: Two passes combine here. Sentence rhythm lengthens the middle medium-variant sentence (lengths shift from `[11, 6, 12, 5, 11, 5]` to `[11, 6, 21, 5, 11, 5]`, stdev 3.04 → 5.61). On top of that, the connector-family budget caps the two-element similarity pool at two emissions inside its trailing window, so the tail follow-on sentences render plain — the old `Similarly,/Likewise,/Similarly,/Likewise,/Similarly,` alternation Matt flagged is gone, leaving only one `Similarly,` and one `Likewise,` across the five follow-ons.

8. Sentence rhythm, component rewrites

Before: The component LoginPanel was touched and revalidated against the current schema. Similarly, the component SessionDrawer was touched. Likewise, the component ProfileCard was touched and revalidated against the current schema. Similarly, the component SettingsModal was touched. Likewise, the component ToastDispatcher was touched and revalidated against the current schema. Similarly, the component BreadcrumbTrail was touched.

After: The component LoginPanel was touched and revalidated against the current schema. Similarly, the component SessionDrawer was touched. Likewise, the component ProfileCard was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The component SettingsModal was touched. The component ToastDispatcher was touched and revalidated against the current schema. The component BreadcrumbTrail was touched.

Why it matters: The cadence improvement still introduces a longer middle sentence, but the connector-family budget also dissolves the patterned `Similarly,/Likewise,/Similarly,...` tail into plain follow-ons, so the prose no longer reads as a two-token alternation loop.

9. Sentence rhythm, function tweaks

Before: The function loadConfig was touched and revalidated against the current schema. Similarly, the function parseConfig was touched. Likewise, the function validateConfig was touched and revalidated against the current schema. Similarly, the function mergeConfig was touched. Likewise, the function writeConfig was touched and revalidated against the current schema. Similarly, the function rotateConfig was touched.

After: The function loadConfig was touched and revalidated against the current schema. Similarly, the function parseConfig was touched. Likewise, the function validateConfig was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The function mergeConfig was touched. The function writeConfig was touched and revalidated against the current schema. The function rotateConfig was touched.

Why it matters: The changed sentence-length contour is still there, and now the connector-family budget removes the residual `Similarly,/Likewise,` alternation in the tail so a repetitive technical update no longer reads like a fixed template loop.

10. Sentence rhythm, page rewrites

Before: The page DashboardPage was touched and revalidated against the current schema. Similarly, the page BillingPage was touched. Likewise, the page ProfilePage was touched and revalidated against the current schema. Similarly, the page SettingsPage was touched. Likewise, the page AdminPage was touched and revalidated against the current schema. Similarly, the page AuditPage was touched.

After: The page DashboardPage was touched and revalidated against the current schema. Similarly, the page BillingPage was touched. Likewise, the page ProfilePage was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The page SettingsPage was touched. The page AdminPage was touched and revalidated against the current schema. The page AuditPage was touched.

Why it matters: The aggregate rhythm fixture now raises cadence stdev from 3.037 to 5.610 across all 60 deterministic sentences (rhythm-off baseline shifted from 2.85 because the de-pattern budget removes the regular connector tokens that previously inflated short-sentence word counts) while preserving max sentence length and event count, and it does so on top of prose that no longer alternates `Similarly,/Likewise,` once the family budget engages.
