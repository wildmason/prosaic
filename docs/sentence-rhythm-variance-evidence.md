# Sentence-rhythm variance — Lane A evidence

Reproduce with `cargo run -p prosaic-core --example sentence_rhythm_demo` (deterministic seed 17, three-variant template registered short / medium / long).

The headline number is at the bottom: aggregate sentence-length stdev across all 60 rendered sentences nearly doubles when the new rhythm penalty is enabled, while max-sentence-length, terminal punctuation, and event/proposition counts are preserved across both columns.

The connector-family budget added in the de-patterning follow-up also changes the surface text in both columns: the two-element similarity pool ("Similarly,", "Likewise,") is capped at two emissions inside its trailing window, so the third onward follow-on sentence renders with no leading connective. That removes the old `Similarly,/Likewise,/Similarly,/Likewise,/Similarly,` alternation Matt flagged in service-shape prose without disturbing rhythm gains — the AFTER column still hits the same five long medium-variant sentences, only the patterned connector tail is gone.

Raw run output follows.

---

Sentence-rhythm variance — 10 before/after narratives (seed 17, max sentence 140).

--- 1. Service rename batch ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The service BillingService was touched and revalidated against the current schema. Similarly, the service PaymentGateway was touched. Likewise, the service RefundProcessor was touched and revalidated against the current schema. The service ReceiptDispatcher was touched. The service InvoiceLedger was touched and revalidated against the current schema. The service AuditTrail was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The service BillingService was touched and revalidated against the current schema. Similarly, the service PaymentGateway was touched. Likewise, the service RefundProcessor was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The service ReceiptDispatcher was touched. The service InvoiceLedger was touched and revalidated against the current schema. The service AuditTrail was touched.

--- 2. Component rewrites ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The component LoginPanel was touched and revalidated against the current schema. Similarly, the component SessionDrawer was touched. Likewise, the component ProfileCard was touched and revalidated against the current schema. The component SettingsModal was touched. The component ToastDispatcher was touched and revalidated against the current schema. The component BreadcrumbTrail was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The component LoginPanel was touched and revalidated against the current schema. Similarly, the component SessionDrawer was touched. Likewise, the component ProfileCard was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The component SettingsModal was touched. The component ToastDispatcher was touched and revalidated against the current schema. The component BreadcrumbTrail was touched.

--- 3. Module migrations ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The module AccountsCore was touched and revalidated against the current schema. Similarly, the module AccountsApi was touched. Likewise, the module AccountsCli was touched and revalidated against the current schema. The module AccountsDocs was touched. The module AccountsFixtures was touched and revalidated against the current schema. The module AccountsBench was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The module AccountsCore was touched and revalidated against the current schema. Similarly, the module AccountsApi was touched. Likewise, the module AccountsCli was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The module AccountsDocs was touched. The module AccountsFixtures was touched and revalidated against the current schema. The module AccountsBench was touched.

--- 4. Class refactors ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The class OrderCart was touched and revalidated against the current schema. Similarly, the class OrderHistory was touched. Likewise, the class OrderInvoice was touched and revalidated against the current schema. The class OrderRefund was touched. The class OrderDispatcher was touched and revalidated against the current schema. The class OrderArchive was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The class OrderCart was touched and revalidated against the current schema. Similarly, the class OrderHistory was touched. Likewise, the class OrderInvoice was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The class OrderRefund was touched. The class OrderDispatcher was touched and revalidated against the current schema. The class OrderArchive was touched.

--- 5. Function tweaks ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The function loadConfig was touched and revalidated against the current schema. Similarly, the function parseConfig was touched. Likewise, the function validateConfig was touched and revalidated against the current schema. The function mergeConfig was touched. The function writeConfig was touched and revalidated against the current schema. The function rotateConfig was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The function loadConfig was touched and revalidated against the current schema. Similarly, the function parseConfig was touched. Likewise, the function validateConfig was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The function mergeConfig was touched. The function writeConfig was touched and revalidated against the current schema. The function rotateConfig was touched.

--- 6. Endpoint reorgs ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The endpoint GetOrders was touched and revalidated against the current schema. Similarly, the endpoint PostOrder was touched. Likewise, the endpoint PatchOrder was touched and revalidated against the current schema. The endpoint DeleteOrder was touched. The endpoint ListOrderItems was touched and revalidated against the current schema. The endpoint PostOrderItem was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The endpoint GetOrders was touched and revalidated against the current schema. Similarly, the endpoint PostOrder was touched. Likewise, the endpoint PatchOrder was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The endpoint DeleteOrder was touched. The endpoint ListOrderItems was touched and revalidated against the current schema. The endpoint PostOrderItem was touched.

--- 7. Pipeline stages ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The stage FetchStage was touched and revalidated against the current schema. Similarly, the stage ParseStage was touched. Likewise, the stage ValidateStage was touched and revalidated against the current schema. The stage TransformStage was touched. The stage PublishStage was touched and revalidated against the current schema. The stage ArchiveStage was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The stage FetchStage was touched and revalidated against the current schema. Similarly, the stage ParseStage was touched. Likewise, the stage ValidateStage was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The stage TransformStage was touched. The stage PublishStage was touched and revalidated against the current schema. The stage ArchiveStage was touched.

--- 8. Workflow audit ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The workflow OnboardingFlow was touched and revalidated against the current schema. Similarly, the workflow OffboardingFlow was touched. Likewise, the workflow PromotionFlow was touched and revalidated against the current schema. The workflow DemotionFlow was touched. The workflow ResetFlow was touched and revalidated against the current schema. The workflow ImpersonationFlow was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The workflow OnboardingFlow was touched and revalidated against the current schema. Similarly, the workflow OffboardingFlow was touched. Likewise, the workflow PromotionFlow was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The workflow DemotionFlow was touched. The workflow ResetFlow was touched and revalidated against the current schema. The workflow ImpersonationFlow was touched.

--- 9. Job restructure ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The job NightlyClean was touched and revalidated against the current schema. Similarly, the job HourlyMetrics was touched. Likewise, the job DailyDigest was touched and revalidated against the current schema. The job WeeklyReport was touched. The job MonthlyClose was touched and revalidated against the current schema. The job AnnualAudit was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The job NightlyClean was touched and revalidated against the current schema. Similarly, the job HourlyMetrics was touched. Likewise, the job DailyDigest was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The job WeeklyReport was touched. The job MonthlyClose was touched and revalidated against the current schema. The job AnnualAudit was touched.

--- 10. Page rewrites ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 5, 11, 5], stdev 3.04):
    The page DashboardPage was touched and revalidated against the current schema. Similarly, the page BillingPage was touched. Likewise, the page ProfilePage was touched and revalidated against the current schema. The page SettingsPage was touched. The page AdminPage was touched and revalidated against the current schema. The page AuditPage was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 5, 11, 5], stdev 5.61):
    The page DashboardPage was touched and revalidated against the current schema. Similarly, the page BillingPage was touched. Likewise, the page ProfilePage was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. The page SettingsPage was touched. The page AdminPage was touched and revalidated against the current schema. The page AuditPage was touched.

=== Aggregate cadence stdev across all 10 narratives ===
BEFORE (rhythm off): stdev 3.037 over 60 sentences
AFTER  (rhythm on) : stdev 5.610 over 60 sentences
