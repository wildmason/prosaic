# Prosaic realism before/after samples

Verification run: `cargo test -p prosaic-core` passed with 709 total passing tests, 0 failed tests, and 2 ignored doctests. Evidence commands used for the rendered samples:

- `cargo run -p prosaic-core --example list_style_continuity_demo`
- `cargo run -p prosaic-core --example sentence_rhythm_demo`
- `connective_variety_uses_longer_recency_window_in_rendered_prose` in `prosaic-core/tests/integration.rs`

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

After: The service BillingService was touched and revalidated against the current schema. Similarly, the service PaymentGateway was touched. Likewise, the service RefundProcessor was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the service ReceiptDispatcher was touched. Likewise, the service InvoiceLedger was touched and revalidated against the current schema. Similarly, the service AuditTrail was touched.

Why it matters: The rhythm-enabled output changes the length pattern from [11, 6, 12, 6, 12, 6] to [11, 6, 21, 6, 12, 6], raising stdev from 2.85 to 5.37 without losing any event.

8. Sentence rhythm, component rewrites

Before: The component LoginPanel was touched and revalidated against the current schema. Similarly, the component SessionDrawer was touched. Likewise, the component ProfileCard was touched and revalidated against the current schema. Similarly, the component SettingsModal was touched. Likewise, the component ToastDispatcher was touched and revalidated against the current schema. Similarly, the component BreadcrumbTrail was touched.

After: The component LoginPanel was touched and revalidated against the current schema. Similarly, the component SessionDrawer was touched. Likewise, the component ProfileCard was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the component SettingsModal was touched. Likewise, the component ToastDispatcher was touched and revalidated against the current schema. Similarly, the component BreadcrumbTrail was touched.

Why it matters: The cadence improvement introduces a longer middle sentence, reducing the mechanical short/medium alternation while keeping the same six propositions.

9. Sentence rhythm, function tweaks

Before: The function loadConfig was touched and revalidated against the current schema. Similarly, the function parseConfig was touched. Likewise, the function validateConfig was touched and revalidated against the current schema. Similarly, the function mergeConfig was touched. Likewise, the function writeConfig was touched and revalidated against the current schema. Similarly, the function rotateConfig was touched.

After: The function loadConfig was touched and revalidated against the current schema. Similarly, the function parseConfig was touched. Likewise, the function validateConfig was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the function mergeConfig was touched. Likewise, the function writeConfig was touched and revalidated against the current schema. Similarly, the function rotateConfig was touched.

Why it matters: The changed sentence-length contour makes a repetitive technical update read less like a fixed template loop.

10. Sentence rhythm, page rewrites

Before: The page DashboardPage was touched and revalidated against the current schema. Similarly, the page BillingPage was touched. Likewise, the page ProfilePage was touched and revalidated against the current schema. Similarly, the page SettingsPage was touched. Likewise, the page AdminPage was touched and revalidated against the current schema. Similarly, the page AuditPage was touched.

After: The page DashboardPage was touched and revalidated against the current schema. Similarly, the page BillingPage was touched. Likewise, the page ProfilePage was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the page SettingsPage was touched. Likewise, the page AdminPage was touched and revalidated against the current schema. Similarly, the page AuditPage was touched.

Why it matters: The aggregate rhythm fixture raises cadence stdev from 2.853 to 5.375 across all 60 deterministic sentences while preserving max sentence length and event count.
