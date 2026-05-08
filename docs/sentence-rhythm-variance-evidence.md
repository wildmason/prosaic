# Sentence-rhythm variance — Lane A evidence

Reproduce with `cargo run -p prosaic-core --example sentence_rhythm_demo` (deterministic seed 17, three-variant template registered short / medium / long).

The headline number is at the bottom: aggregate sentence-length stdev across all 60 rendered sentences nearly doubles when the new rhythm penalty is enabled, while max-sentence-length, terminal punctuation, and event/proposition counts are preserved across both columns.

Raw run output follows.

---

Sentence-rhythm variance — 10 before/after narratives (seed 17, max sentence 140).

--- 1. Service rename batch ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The service BillingService was touched and revalidated against the current schema. Similarly, the service PaymentGateway was touched. Likewise, the service RefundProcessor was touched and revalidated against the current schema. Similarly, the service ReceiptDispatcher was touched. Likewise, the service InvoiceLedger was touched and revalidated against the current schema. Similarly, the service AuditTrail was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The service BillingService was touched and revalidated against the current schema. Similarly, the service PaymentGateway was touched. Likewise, the service RefundProcessor was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the service ReceiptDispatcher was touched. Likewise, the service InvoiceLedger was touched and revalidated against the current schema. Similarly, the service AuditTrail was touched.

--- 2. Component rewrites ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The component LoginPanel was touched and revalidated against the current schema. Similarly, the component SessionDrawer was touched. Likewise, the component ProfileCard was touched and revalidated against the current schema. Similarly, the component SettingsModal was touched. Likewise, the component ToastDispatcher was touched and revalidated against the current schema. Similarly, the component BreadcrumbTrail was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The component LoginPanel was touched and revalidated against the current schema. Similarly, the component SessionDrawer was touched. Likewise, the component ProfileCard was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the component SettingsModal was touched. Likewise, the component ToastDispatcher was touched and revalidated against the current schema. Similarly, the component BreadcrumbTrail was touched.

--- 3. Module migrations ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The module AccountsCore was touched and revalidated against the current schema. Similarly, the module AccountsApi was touched. Likewise, the module AccountsCli was touched and revalidated against the current schema. Similarly, the module AccountsDocs was touched. Likewise, the module AccountsFixtures was touched and revalidated against the current schema. Similarly, the module AccountsBench was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The module AccountsCore was touched and revalidated against the current schema. Similarly, the module AccountsApi was touched. Likewise, the module AccountsCli was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the module AccountsDocs was touched. Likewise, the module AccountsFixtures was touched and revalidated against the current schema. Similarly, the module AccountsBench was touched.

--- 4. Class refactors ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The class OrderCart was touched and revalidated against the current schema. Similarly, the class OrderHistory was touched. Likewise, the class OrderInvoice was touched and revalidated against the current schema. Similarly, the class OrderRefund was touched. Likewise, the class OrderDispatcher was touched and revalidated against the current schema. Similarly, the class OrderArchive was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The class OrderCart was touched and revalidated against the current schema. Similarly, the class OrderHistory was touched. Likewise, the class OrderInvoice was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the class OrderRefund was touched. Likewise, the class OrderDispatcher was touched and revalidated against the current schema. Similarly, the class OrderArchive was touched.

--- 5. Function tweaks ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The function loadConfig was touched and revalidated against the current schema. Similarly, the function parseConfig was touched. Likewise, the function validateConfig was touched and revalidated against the current schema. Similarly, the function mergeConfig was touched. Likewise, the function writeConfig was touched and revalidated against the current schema. Similarly, the function rotateConfig was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The function loadConfig was touched and revalidated against the current schema. Similarly, the function parseConfig was touched. Likewise, the function validateConfig was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the function mergeConfig was touched. Likewise, the function writeConfig was touched and revalidated against the current schema. Similarly, the function rotateConfig was touched.

--- 6. Endpoint reorgs ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The endpoint GetOrders was touched and revalidated against the current schema. Similarly, the endpoint PostOrder was touched. Likewise, the endpoint PatchOrder was touched and revalidated against the current schema. Similarly, the endpoint DeleteOrder was touched. Likewise, the endpoint ListOrderItems was touched and revalidated against the current schema. Similarly, the endpoint PostOrderItem was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The endpoint GetOrders was touched and revalidated against the current schema. Similarly, the endpoint PostOrder was touched. Likewise, the endpoint PatchOrder was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the endpoint DeleteOrder was touched. Likewise, the endpoint ListOrderItems was touched and revalidated against the current schema. Similarly, the endpoint PostOrderItem was touched.

--- 7. Pipeline stages ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The stage FetchStage was touched and revalidated against the current schema. Similarly, the stage ParseStage was touched. Likewise, the stage ValidateStage was touched and revalidated against the current schema. Similarly, the stage TransformStage was touched. Likewise, the stage PublishStage was touched and revalidated against the current schema. Similarly, the stage ArchiveStage was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The stage FetchStage was touched and revalidated against the current schema. Similarly, the stage ParseStage was touched. Likewise, the stage ValidateStage was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the stage TransformStage was touched. Likewise, the stage PublishStage was touched and revalidated against the current schema. Similarly, the stage ArchiveStage was touched.

--- 8. Workflow audit ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The workflow OnboardingFlow was touched and revalidated against the current schema. Similarly, the workflow OffboardingFlow was touched. Likewise, the workflow PromotionFlow was touched and revalidated against the current schema. Similarly, the workflow DemotionFlow was touched. Likewise, the workflow ResetFlow was touched and revalidated against the current schema. Similarly, the workflow ImpersonationFlow was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The workflow OnboardingFlow was touched and revalidated against the current schema. Similarly, the workflow OffboardingFlow was touched. Likewise, the workflow PromotionFlow was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the workflow DemotionFlow was touched. Likewise, the workflow ResetFlow was touched and revalidated against the current schema. Similarly, the workflow ImpersonationFlow was touched.

--- 9. Job restructure ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The job NightlyClean was touched and revalidated against the current schema. Similarly, the job HourlyMetrics was touched. Likewise, the job DailyDigest was touched and revalidated against the current schema. Similarly, the job WeeklyReport was touched. Likewise, the job MonthlyClose was touched and revalidated against the current schema. Similarly, the job AnnualAudit was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The job NightlyClean was touched and revalidated against the current schema. Similarly, the job HourlyMetrics was touched. Likewise, the job DailyDigest was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the job WeeklyReport was touched. Likewise, the job MonthlyClose was touched and revalidated against the current schema. Similarly, the job AnnualAudit was touched.

--- 10. Page rewrites ---
BEFORE (sentence_rhythm=false, lengths [11, 6, 12, 6, 12, 6], stdev 2.85):
    The page DashboardPage was touched and revalidated against the current schema. Similarly, the page BillingPage was touched. Likewise, the page ProfilePage was touched and revalidated against the current schema. Similarly, the page SettingsPage was touched. Likewise, the page AdminPage was touched and revalidated against the current schema. Similarly, the page AuditPage was touched.
AFTER  (sentence_rhythm=true,  lengths [11, 6, 21, 6, 12, 6], stdev 5.37):
    The page DashboardPage was touched and revalidated against the current schema. Similarly, the page BillingPage was touched. Likewise, the page ProfilePage was touched after the routine sweep, revalidated against the current schema, and recorded in the engineering ledger. Similarly, the page SettingsPage was touched. Likewise, the page AdminPage was touched and revalidated against the current schema. Similarly, the page AuditPage was touched.

=== Aggregate cadence stdev across all 10 narratives ===
BEFORE (rhythm off): stdev 2.853 over 60 sentences
AFTER  (rhythm on) : stdev 5.375 over 60 sentences
