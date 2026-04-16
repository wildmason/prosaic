use prosaic_core::{
    Context, DocumentPlan, Engine, GroupingStrategy, Session, Strictness, Variation, ctx,
};
use prosaic_grammar_en::English;

fn main() {
    println!("=== NLG Engine — Cross-Domain Showcase ===\n");

    monitoring_alerts();
    deployment_events();
    user_activity_digest();
    infrastructure_changes();
    business_metrics();
    incident_narrative();
    database_operations();
    security_audit();
}

fn header(title: &str) {
    println!("── {title} ──\n");
}

fn show(label: &str, text: &str) {
    println!("  {label}:\n    \"{text}\"\n");
}

// ── Monitoring & Alerting ────────────────────────────────────────────────

fn monitoring_alerts() {
    header("Monitoring & Alerting");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);

    engine.register_template(
        "alert.threshold_breached",
        "The {metric} on {service} {severity|choose: critical=has critically exceeded, warning=has exceeded, default=is approaching} its threshold, reaching {current_value} against a limit of {threshold}{?duration} sustained for {duration}{/?}",
    ).unwrap();

    engine.register_template(
        "alert.error_spike",
        "{service} is experiencing {error_count|quantify} errors per minute{?error_type} of type {error_type}{/?}{?affected_endpoints}, impacting {affected_endpoints|truncate:3|join:bracketed}{/?}",
    ).unwrap();

    engine.register_template(
        "alert.recovery",
        "{service} has recovered from {incident_type}{?downtime_minutes} after {downtime_minutes} {downtime_minutes|pluralize:minute} of degradation{/?}. All health checks are now passing",
    ).unwrap();

    engine.register_template(
        "alert.anomaly",
        "Anomalous behaviour detected on {service}: {metric} is {deviation}% {direction|choose: above=above, below=below, default=deviating from} the 7-day rolling average{?confidence} ({confidence|hedge} significant){/?}",
    ).unwrap();

    let mut session = Session::new();

    show(
        "CPU threshold (critical)",
        &engine
            .render(
                &mut session,
                "alert.threshold_breached",
                &ctx! {
                    metric: "CPU utilisation",
                    service: "api-gateway-prod",
                    severity: "critical",
                    current_value: "98.2%",
                    threshold: "85%",
                    duration: "12 minutes",
                },
            )
            .unwrap(),
    );

    session.reset();
    show("Error spike", &engine.render(&mut session, "alert.error_spike", &ctx! {
        service: "payment-service",
        error_count: 847,
        error_type: "ConnectionTimeout",
        affected_endpoints: vec!["POST /checkout".to_string(), "POST /refund".to_string(), "GET /invoice".to_string(), "POST /subscription".to_string()],
    }).unwrap());

    session.reset();
    show(
        "Recovery",
        &engine
            .render(
                &mut session,
                "alert.recovery",
                &ctx! {
                    service: "auth-service",
                    incident_type: "elevated 5xx error rate",
                    downtime_minutes: 7,
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Anomaly (high confidence)",
        &engine
            .render(
                &mut session,
                "alert.anomaly",
                &ctx! {
                    service: "recommendation-engine",
                    metric: "p99 latency",
                    deviation: "340",
                    direction: "above",
                    confidence: 92,
                },
            )
            .unwrap(),
    );
}

// ── Deployment Events ────────────────────────────────────────────────────

fn deployment_events() {
    header("Deployment Events");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);

    engine.register_template(
        "deploy.started",
        "Deployment of {service} version {version} to {environment} has been initiated by {deployer}{?strategy} using {strategy} strategy{/?}",
    ).unwrap();

    engine.register_template(
        "deploy.canary_promoted",
        "Canary deployment of {service} {version} to {environment} has been promoted to full rollout after {canary_duration} with {canary_error_rate} error rate across {canary_traffic}% of traffic",
    ).unwrap();

    engine.register_template(
        "deploy.rollback",
        "{service} in {environment} has been rolled back from {bad_version} to {good_version}{?reason} due to {reason}{/?}{?affected_users}, impacting {affected_users|quantify} {affected_users|pluralize:user}{/?}",
    ).unwrap();

    engine.register_template(
        "deploy.completed",
        "{service} {version} is now live in {environment}{?instances} across {instances} {instances|pluralize:instance}{/?}. Deployment took {duration} with zero-downtime",
    ).unwrap();

    let mut session = Session::new();

    let events: Vec<(&str, Context)> = vec![
        (
            "deploy.started",
            ctx! {
                service: "billing-api",
                version: "3.4.1",
                environment: "production",
                deployer: "Alice Chen",
                strategy: "blue-green",
            },
        ),
        (
            "deploy.canary_promoted",
            ctx! {
                service: "billing-api",
                version: "3.4.1",
                environment: "production",
                canary_duration: "45 minutes",
                canary_error_rate: "0.02%",
                canary_traffic: 10,
            },
        ),
        (
            "deploy.completed",
            ctx! {
                service: "billing-api",
                version: "3.4.1",
                environment: "production",
                instances: 12,
                duration: "8 minutes",
            },
        ),
    ];

    let narrative = engine.render_batch(&mut session, &events).unwrap();
    println!("  Deployment lifecycle (3 events → batch narrative):\n");
    for line in narrative.lines() {
        println!("    {line}");
    }
    println!();

    session.reset();
    show(
        "Rollback",
        &engine
            .render(
                &mut session,
                "deploy.rollback",
                &ctx! {
                    service: "checkout-service",
                    environment: "production",
                    bad_version: "2.1.0",
                    good_version: "2.0.9",
                    reason: "elevated 5xx error rate exceeding 2% SLO",
                    affected_users: 3400,
                },
            )
            .unwrap(),
    );
}

// ── User Activity Digest ─────────────────────────────────────────────────

fn user_activity_digest() {
    header("User Activity Digest");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);

    engine.register_template(
        "user.signup",
        "{count|quantify} new {count|pluralize:user} signed up{?source} via {source}{/?}{?plan}, choosing the {plan} plan{/?}",
    ).unwrap();

    engine.register_template(
        "user.churn",
        "{count} {count|pluralize:account} churned this period{?top_reason}, primarily due to {top_reason}{/?}{?retention_rate} (retention rate: {retention_rate}){/?}",
    ).unwrap();

    engine.register_template(
        "user.milestone",
        "{user_name} reached the {milestone} milestone{?value}, hitting {value}{/?}{?days_since_signup} just {days_since_signup} {days_since_signup|pluralize:day} after signing up{/?}",
    ).unwrap();

    engine.register_template(
        "user.feature_adoption",
        "{feature_name} adoption reached {adoption_rate} this week{?change} ({change} from last week){/?}{?power_users}, with {power_users|quantify} power {power_users|pluralize:user} accounting for {power_user_pct} of usage{/?}",
    ).unwrap();

    let mut session = Session::new();

    show(
        "Signups",
        &engine
            .render(
                &mut session,
                "user.signup",
                &ctx! {
                    count: 234,
                    source: "organic search",
                    plan: "Professional",
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Churn",
        &engine
            .render(
                &mut session,
                "user.churn",
                &ctx! {
                    count: 18,
                    top_reason: "pricing concerns",
                    retention_rate: "94.2%",
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Milestone",
        &engine
            .render(
                &mut session,
                "user.milestone",
                &ctx! {
                    user_name: "Acme Corp",
                    milestone: "1000th API call",
                    value: "1,024 calls",
                    days_since_signup: 3,
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Feature adoption",
        &engine
            .render(
                &mut session,
                "user.feature_adoption",
                &ctx! {
                    feature_name: "Batch Export",
                    adoption_rate: "37%",
                    change: "+8%",
                    power_users: 42,
                    power_user_pct: "61%",
                },
            )
            .unwrap(),
    );
}

// ── Infrastructure Changes ───────────────────────────────────────────────

fn infrastructure_changes() {
    header("Infrastructure Changes");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Seeded(42));

    engine.register_template(
        "infra.scaled",
        "The {resource_type} {resource_name} in {region} was scaled from {old_count} to {new_count} {new_count|pluralize:instance}{?reason} in response to {reason}{/?}",
    ).unwrap();

    engine.register_template(
        "infra.cert_expiry",
        "The TLS certificate for {domain} expires in {days_until} {days_until|pluralize:day}{?auto_renew}; auto-renewal is {auto_renew|choose: 1=enabled, default=disabled}{/?}",
    ).unwrap();

    engine.register_template(
        "infra.cost_alert",
        "{resource_type} spend in {account} has reached {current_spend} this billing period, which is {percentage_of_budget}% of the monthly budget{?projected_overage} with a projected overage of {projected_overage}{/?}",
    ).unwrap();

    engine.register_template(
        "infra.region_failover",
        "Traffic for {service} has been failed over from {primary_region} to {failover_region}{?trigger} triggered by {trigger}{/?}. Estimated recovery: {eta}",
    ).unwrap();

    let mut session = Session::new();

    show(
        "Auto-scale",
        &engine
            .render(
                &mut session,
                "infra.scaled",
                &ctx! {
                    resource_type: "ECS cluster",
                    resource_name: "api-workers",
                    region: "us-east-1",
                    old_count: 4,
                    new_count: 12,
                    reason: "CPU utilisation exceeding 80% for 5 minutes",
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Cert expiry warning",
        &engine
            .render(
                &mut session,
                "infra.cert_expiry",
                &ctx! {
                    domain: "*.wildmason.com",
                    days_until: 14,
                    auto_renew: 1,
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Cost alert",
        &engine
            .render(
                &mut session,
                "infra.cost_alert",
                &ctx! {
                    resource_type: "Compute",
                    account: "production-main",
                    current_spend: "$14,200",
                    percentage_of_budget: 87,
                    projected_overage: "$2,300",
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Region failover",
        &engine
            .render(
                &mut session,
                "infra.region_failover",
                &ctx! {
                    service: "payment-gateway",
                    primary_region: "us-east-1",
                    failover_region: "eu-west-1",
                    trigger: "health check failures exceeding threshold",
                    eta: "15 minutes",
                },
            )
            .unwrap(),
    );
}

// ── Business Metrics ─────────────────────────────────────────────────────

fn business_metrics() {
    header("Business Metrics");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);

    engine.register_template(
        "metrics.revenue",
        "{period} revenue reached {amount}, {direction|choose: up=up, down=down, default=flat} {change_pct} from the prior period{?top_driver}. The primary driver was {top_driver}{/?}",
    ).unwrap();

    engine.register_template(
        "metrics.conversion",
        "The {funnel_stage} conversion rate {direction|choose: improved=improved to, declined=declined to, default=held at} {rate}{?delta} ({delta} vs last period){/?}{?segment} among {segment} users{/?}",
    ).unwrap();

    engine.register_template(
        "metrics.sla_compliance",
        "SLA compliance for {service} stood at {compliance_rate} this period{?violations}, with {violations} {violations|pluralize:violation} recorded{/?}{?worst_incident}. The most significant incident was {worst_incident}{/?}",
    ).unwrap();

    let mut session = Session::new();

    show(
        "Revenue",
        &engine
            .render(
                &mut session,
                "metrics.revenue",
                &ctx! {
                    period: "Q1 2026",
                    amount: "$2.4M",
                    direction: "up",
                    change_pct: "18%",
                    top_driver: "enterprise tier expansion in EMEA",
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Conversion",
        &engine
            .render(
                &mut session,
                "metrics.conversion",
                &ctx! {
                    funnel_stage: "trial-to-paid",
                    direction: "improved",
                    rate: "14.2%",
                    delta: "+2.1pp",
                    segment: "enterprise",
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "SLA compliance",
        &engine
            .render(
                &mut session,
                "metrics.sla_compliance",
                &ctx! {
                    service: "Core API",
                    compliance_rate: "99.93%",
                    violations: 2,
                    worst_incident: "a 4-minute outage on March 12 caused by a database failover",
                },
            )
            .unwrap(),
    );
}

// ── Incident Narrative ───────────────────────────────────────────────────

fn incident_narrative() {
    header("Incident Narrative (Multi-Event Document Plan)");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);

    engine
        .register_template(
            "incident.detected",
            "An incident was detected on {service} at {timestamp}: {description}",
        )
        .unwrap();

    engine.register_template(
        "incident.impact",
        "The incident affected {affected_users|quantify} {affected_users|pluralize:user} across {affected_regions|join}{?error_rate}, with error rates reaching {error_rate}{/?}",
    ).unwrap();

    engine
        .register_template(
            "incident.mitigation",
            "{responder} applied {action}{?duration} within {duration} of detection{/?}",
        )
        .unwrap();

    engine.register_template(
        "incident.resolved",
        "The incident was resolved at {timestamp}{?root_cause}. Root cause: {root_cause}{/?}{?total_duration}. Total duration: {total_duration}{/?}",
    ).unwrap();

    let events: Vec<(&str, Context)> = vec![
        (
            "incident.detected",
            ctx! {
                service: "checkout-service",
                timestamp: "14:23 UTC",
                description: "elevated 5xx error rate exceeding 5% SLO threshold",
            },
        ),
        (
            "incident.impact",
            ctx! {
                affected_users: 12000,
                affected_regions: vec!["us-east-1".to_string(), "eu-west-1".to_string()],
                error_rate: "8.3%",
            },
        ),
        (
            "incident.mitigation",
            ctx! {
                responder: "Alice Chen",
                action: "a connection pool size increase from 50 to 200",
                duration: "6 minutes",
            },
        ),
        (
            "incident.resolved",
            ctx! {
                timestamp: "14:41 UTC",
                root_cause: "connection pool exhaustion under sustained Black Friday traffic",
                total_duration: "18 minutes",
            },
        ),
    ];

    let mut session = Session::new();
    let plan = DocumentPlan::from_events_grouped(&events, &engine, GroupingStrategy::ByEntity);
    let narrative = plan.render(&engine, &mut session).unwrap();

    println!("  Four incident events → structured narrative:\n");
    for line in narrative.lines() {
        println!("    {line}");
    }
    println!();
}

// ── Database Operations ──────────────────────────────────────────────────

fn database_operations() {
    header("Database Operations");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);

    engine.register_template(
        "db.migration_applied",
        "Migration {migration_name} was applied to {database} in {duration}{?tables_affected}, touching {tables_affected} {tables_affected|pluralize:table}{/?}{?rows_affected} and updating {rows_affected|quantify} {rows_affected|pluralize:row}{/?}",
    ).unwrap();

    engine.register_template(
        "db.slow_query",
        "A slow query was detected on {database}: {query_fingerprint} took {duration}{?rows_scanned} scanning {rows_scanned|quantify} {rows_scanned|pluralize:row}{/?}{?missing_index}. Suggested fix: add an index on {missing_index}{/?}",
    ).unwrap();

    engine.register_template(
        "db.backup_completed",
        "Backup of {database} completed in {duration}, producing a {size} snapshot{?retention}. Retention policy: {retention}{/?}",
    ).unwrap();

    let mut session = Session::new();

    show(
        "Migration",
        &engine
            .render(
                &mut session,
                "db.migration_applied",
                &ctx! {
                    migration_name: "20260415_add_audit_log_table",
                    database: "production-primary",
                    duration: "3.2 seconds",
                    tables_affected: 2,
                    rows_affected: 0,
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Slow query",
        &engine
            .render(
                &mut session,
                "db.slow_query",
                &ctx! {
                    database: "analytics-replica",
                    query_fingerprint: "SELECT * FROM events WHERE user_id = ? AND created_at > ?",
                    duration: "4.7 seconds",
                    rows_scanned: 2400000,
                    missing_index: "events(user_id, created_at)",
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Backup",
        &engine
            .render(
                &mut session,
                "db.backup_completed",
                &ctx! {
                    database: "production-primary",
                    duration: "12 minutes",
                    size: "47 GB",
                    retention: "30 days with daily snapshots",
                },
            )
            .unwrap(),
    );
}

// ── Security Audit ───────────────────────────────────────────────────────

fn security_audit() {
    header("Security Audit");

    let mut engine = Engine::new(English::new())
        .strictness(Strictness::Silent)
        .variation(Variation::Fixed);

    engine.register_template(
        "security.vulnerability_found",
        "A {severity|choose: critical=critical, high=high-severity, medium=medium-severity, low=low-severity, default=unclassified} vulnerability was found in {component}: {description}{?cve} ({cve}){/?}",
    ).unwrap();

    engine.register_template(
        "security.access_anomaly",
        "Unusual access pattern detected for {principal}: {action_count} {action_count|pluralize:action} on {resource}{?source_ip} from {source_ip}{/?}{?time_window} within {time_window}{/?}",
    ).unwrap();

    engine.register_template(
        "security.dependency_audit",
        "Dependency audit of {project} found {total_issues} {total_issues|pluralize:issue}{?critical}: {critical} critical{/?}{?high}, {high} high{/?}{?outdated}. {outdated} {outdated|pluralize:dependency} {outdated|choose: 1=is, default=are} outdated{/?}",
    ).unwrap();

    let mut session = Session::new();

    show(
        "Vulnerability (critical)",
        &engine
            .render(
                &mut session,
                "security.vulnerability_found",
                &ctx! {
                    severity: "critical",
                    component: "openssl 3.0.2",
                    description: "buffer overflow in X.509 certificate verification",
                    cve: "CVE-2026-1234",
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Access anomaly",
        &engine
            .render(
                &mut session,
                "security.access_anomaly",
                &ctx! {
                    principal: "service-account-47",
                    action_count: 3400,
                    resource: "secrets-manager",
                    source_ip: "203.0.113.42",
                    time_window: "5 minutes",
                },
            )
            .unwrap(),
    );

    session.reset();
    show(
        "Dependency audit",
        &engine
            .render(
                &mut session,
                "security.dependency_audit",
                &ctx! {
                    project: "billing-api",
                    total_issues: 14,
                    critical: 2,
                    high: 5,
                    outdated: 7,
                },
            )
            .unwrap(),
    );
}
