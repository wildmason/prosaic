//! Realistic-use-case samples — what actual Prosaic output looks like
//! when wired into the kinds of pipelines the library is built for.
//!
//! Templates are registered inline so the example is self-contained and
//! the relationship between input → template → output is visible.

use prosaic_core::{Context, DocumentPlan, Engine, Salience, Session, Strictness, Value, Variation};
use prosaic_grammar_en::English;

fn ctx(pairs: &[(&str, Value)]) -> Context {
    let mut c = Context::new();
    for (k, v) in pairs {
        c.insert(*k, v.clone());
    }
    c
}

fn s(v: &str) -> Value {
    Value::String(v.into())
}
fn n(v: i64) -> Value {
    Value::Number(v)
}
fn list(items: &[&str]) -> Value {
    Value::List(items.iter().map(|x| (*x).to_string()).collect())
}

fn header(title: &str) {
    println!("\n━━━ {title} ━━━");
}

fn main() {
    // ── Scenario 1: Release notes ──────────────────────────────────────
    header("Release notes");
    {
        let mut e = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Seeded(42));
        e.register_template_at(
            "release.tagged",
            "Version {version}{?title} (\u{201c}{title}\u{201d}){/?} \
             has been tagged{?date} on {date}{/?}",
            Salience::High,
        )
        .unwrap();
        e.register_template_at(
            "release.feature",
            "{name|refer} introduces {feature}{?description} \u{2014} {description}{/?}",
            Salience::Medium,
        )
        .unwrap();
        e.register_template_at(
            "release.bugfix",
            "Fixed: {description}{?issue} (issue #{issue}){/?}",
            Salience::Low,
        )
        .unwrap();
        e.register_template_at(
            "release.security",
            "Security fix: {description}{?cve} \u{2014} {cve}{/?}",
            Salience::High,
        )
        .unwrap();
        e.register_template_at(
            "release.contributors",
            "{count} {count|pluralize:contributor} shipped this release\
             {?names}, including {names|truncate:3}{/?}",
            Salience::Medium,
        )
        .unwrap();

        let events = vec![
            (
                "release.tagged",
                ctx(&[
                    ("name", s("v2.4.0")),
                    ("entity_type", s("release")),
                    ("version", s("2.4.0")),
                    ("title", s("Q2 stability release")),
                    ("date", s("2026-05-09")),
                ]),
            ),
            (
                "release.feature",
                ctx(&[
                    ("name", s("v2.4.0")),
                    ("entity_type", s("release")),
                    ("feature", s("audit-log streaming")),
                    ("description", s("real-time event delivery over WebSocket")),
                ]),
            ),
            (
                "release.feature",
                ctx(&[
                    ("name", s("v2.4.0")),
                    ("entity_type", s("release")),
                    ("feature", s("API rate-limit headers")),
                ]),
            ),
            (
                "release.bugfix",
                ctx(&[
                    ("description", s("session cookies no longer leak across tenants")),
                    ("issue", n(2148)),
                ]),
            ),
            (
                "release.security",
                ctx(&[
                    ("description", s("path traversal in attachment downloads")),
                    ("cve", s("CVE-2026-1834")),
                ]),
            ),
            (
                "release.contributors",
                ctx(&[
                    ("count", n(7)),
                    ("names", list(&["Alice", "Bao", "Carmen", "Devon", "Esi"])),
                ]),
            ),
        ];
        let plan = DocumentPlan::from_events(&events, &e);
        println!("{}", plan.render(&e, &mut Session::new()).unwrap());
    }

    // ── Scenario 2: PR digest ─────────────────────────────────────────
    header("Pull-request digest");
    {
        let mut e = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Seeded(11));
        e.register_template_at(
            "pr.summary",
            "PR #{number} by {author}: \u{201c}{title}\u{201d} \u{2014} \
             {commits} {commits|pluralize:commit} across {files} {files|pluralize:file}",
            Salience::High,
        )
        .unwrap();
        e.register_template_at(
            "pr.scope",
            "It spans {areas|truncate:3|join}",
            Salience::Medium,
        )
        .unwrap();
        e.register_template_at(
            "pr.review",
            "Review state: {approvals} {approvals|pluralize:approval}\
             {?changes}, {changes} {changes|pluralize:request} for changes{/?}\
             {?pending}, and {pending} pending {pending|pluralize:review}{/?}",
            Salience::Medium,
        )
        .unwrap();
        e.register_template_at(
            "pr.ci",
            "Continuous integration reports {status} ({passed}/{total} checks green)",
            Salience::Low,
        )
        .unwrap();
        e.register_template_at(
            "pr.blocker",
            "Merge is blocked by {blocker}",
            Salience::High,
        )
        .unwrap();

        let events = vec![
            (
                "pr.summary",
                ctx(&[
                    ("name", s("PR #412")),
                    ("entity_type", s("pull request")),
                    ("number", n(412)),
                    ("author", s("Devon")),
                    ("title", s("Replace blocking I/O with async tokio runtime")),
                    ("commits", n(14)),
                    ("files", n(31)),
                ]),
            ),
            (
                "pr.scope",
                ctx(&[
                    ("name", s("PR #412")),
                    ("entity_type", s("pull request")),
                    (
                        "areas",
                        list(&["api/handlers", "core/runtime", "tests/integration"]),
                    ),
                ]),
            ),
            (
                "pr.review",
                ctx(&[
                    ("name", s("PR #412")),
                    ("entity_type", s("pull request")),
                    ("approvals", n(2)),
                    ("changes", n(1)),
                    ("pending", n(1)),
                ]),
            ),
            (
                "pr.ci",
                ctx(&[
                    ("name", s("PR #412")),
                    ("entity_type", s("pull request")),
                    ("status", s("passing")),
                    ("passed", n(8)),
                    ("total", n(8)),
                ]),
            ),
            (
                "pr.blocker",
                ctx(&[
                    ("name", s("PR #412")),
                    ("entity_type", s("pull request")),
                    ("blocker", s("one outstanding request for changes")),
                ]),
            ),
        ];
        let plan = DocumentPlan::from_events(&events, &e);
        println!("{}", plan.render(&e, &mut Session::new()).unwrap());
    }

    // ── Scenario 3: Code-change activity summary ──────────────────────
    header("Code-change activity summary");
    {
        let mut e = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Seeded(7));
        prosaic_vocab_code::register(&mut e).unwrap();

        let events = vec![
            (
                "code.added",
                ctx(&[
                    ("entity_type", s("module")),
                    ("name", s("billing::tax")),
                    ("location", s("src/billing/tax.rs")),
                ]),
            ),
            (
                "code.modified",
                ctx(&[
                    ("entity_type", s("function")),
                    ("name", s("apply_promo_code")),
                    ("location", s("src/billing/promotions.rs")),
                    ("consumer_count", n(6)),
                ]),
            ),
            (
                "code.modified",
                ctx(&[
                    ("entity_type", s("function")),
                    ("name", s("apply_promo_code")),
                ]),
            ),
            (
                "code.renamed",
                ctx(&[
                    ("entity_type", s("class")),
                    ("name", s("PriceCalculator")),
                    ("old_name", s("PriceCalculator")),
                    ("new_name", s("InvoicePricer")),
                    ("consumer_count", n(14)),
                ]),
            ),
            (
                "code.deleted",
                ctx(&[
                    ("entity_type", s("module")),
                    ("name", s("legacy::stripe_v1")),
                ]),
            ),
        ];
        let plan = DocumentPlan::from_events(&events, &e);
        println!("{}", plan.render(&e, &mut Session::new()).unwrap());
    }

    // ── Scenario 4: Audit-log narrative ───────────────────────────────
    header("Audit-log narrative");
    {
        let mut e = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Seeded(3));
        e.register_template_at(
            "audit.login",
            "{actor} signed in from {ip}",
            Salience::Low,
        )
        .unwrap();
        e.register_template_at(
            "audit.permission_grant",
            "{actor} granted {permission} on {resource} to {target}",
            Salience::Medium,
        )
        .unwrap();
        e.register_template_at(
            "audit.config_change",
            "{actor} changed {setting} from \u{201c}{from}\u{201d} \
             to \u{201c}{to}\u{201d}",
            Salience::Medium,
        )
        .unwrap();
        e.register_template_at(
            "audit.export",
            "{actor} exported {row_count} {row_count|pluralize:row} from {dataset}",
            Salience::High,
        )
        .unwrap();
        e.register_template_at(
            "audit.logout",
            "{actor} signed out",
            Salience::Low,
        )
        .unwrap();

        let actor_alice = ctx(&[
            ("actor", s("Alice")),
            ("name", s("Alice")),
            ("entity_type", s("user")),
        ]);
        let actor_bao = ctx(&[
            ("actor", s("Bao")),
            ("name", s("Bao")),
            ("entity_type", s("user")),
        ]);

        let mut alice_login = actor_alice.clone();
        alice_login.insert("ip", s("203.0.113.42"));
        let mut alice_grant = actor_alice.clone();
        alice_grant.insert("permission", s("read:billing"));
        alice_grant.insert("resource", s("workspace acme-corp"));
        alice_grant.insert("target", s("Bao"));
        let mut alice_config = actor_alice.clone();
        alice_config.insert("setting", s("session timeout"));
        alice_config.insert("from", s("30m"));
        alice_config.insert("to", s("8h"));
        let mut bao_export = actor_bao.clone();
        bao_export.insert("row_count", n(14823));
        bao_export.insert("dataset", s("customer_invoices"));
        let alice_logout = actor_alice.clone();

        let events = vec![
            ("audit.login", alice_login),
            ("audit.permission_grant", alice_grant),
            ("audit.config_change", alice_config),
            ("audit.export", bao_export),
            ("audit.logout", alice_logout),
        ];
        let plan = DocumentPlan::from_events(&events, &e);
        println!("{}", plan.render(&e, &mut Session::new()).unwrap());
    }

    // ── Scenario 5: Incident timeline ─────────────────────────────────
    header("Incident timeline narrative");
    {
        let mut e = Engine::new(English::new())
            .strictness(Strictness::Strict)
            .variation(Variation::Seeded(19));
        e.register_template_at(
            "incident.opened",
            "{name|refer} was opened in response to a {severity} \
             customer-facing incident affecting {tenant_count} tenants",
            Salience::High,
        )
        .unwrap();
        e.register_template_at(
            "incident.escalated",
            "{name|refer} was escalated to {to}",
            Salience::Medium,
        )
        .unwrap();
        e.register_template_at(
            "incident.mitigated",
            "{name|refer} was mitigated by {action}",
            Salience::Medium,
        )
        .unwrap();
        e.register_template_at(
            "incident.resolved",
            "{name|refer} was resolved after {duration}",
            Salience::Medium,
        )
        .unwrap();

        let base = ctx(&[
            ("name", s("Incident INC-2641")),
            ("entity_type", s("incident")),
            ("severity", s("Sev-2")),
            ("tenant_count", n(11)),
        ]);
        let mut esc = base.clone();
        esc.insert("to", s("on-call platform engineer"));
        let mut mit = base.clone();
        mit.insert("action", s("rolling back deploy abc1234"));
        let mut res = base.clone();
        res.insert("duration", s("47 minutes"));

        let events = vec![
            ("incident.opened", base),
            ("incident.escalated", esc),
            ("incident.mitigated", mit),
            ("incident.resolved", res),
        ];
        let plan = DocumentPlan::from_events(&events, &e);
        println!("{}", plan.render(&e, &mut Session::new()).unwrap());
    }
}
