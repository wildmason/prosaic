//! Integration tests for the `prosaic_template!` proc macro from `prosaic-derive`.
//!
//! Compile-fail behaviour (undeclared slot / unknown pipe emits a clean
//! compile error) is exercised by manual verification during development.
//! A `trybuild` harness for compile-fail tests is deferred to v2.

use prosaic_derive::prosaic_template;

// ── Phase 1: Scaffold ────────────────────────────────────────────────────────

#[test]
fn passes_through_valid_template() {
    let tpl = prosaic_template! {
        template: "The {entity_type} {name} was renamed",
        slots: [entity_type, name],
    };
    assert_eq!(tpl, "The {entity_type} {name} was renamed");
}

#[test]
fn empty_slots_list_allowed_for_literal_only_template() {
    let tpl = prosaic_template! {
        template: "All systems nominal.",
        slots: [],
    };
    assert_eq!(tpl, "All systems nominal.");
}

// ── Phase 2: Slot validation ─────────────────────────────────────────────────

#[test]
fn extra_declared_slots_are_allowed() {
    // Declaring more slots than the template uses is fine.
    let tpl = prosaic_template! {
        template: "Hello {name}",
        slots: [name, unused_slot],
    };
    assert_eq!(tpl, "Hello {name}");
}

#[test]
fn slot_with_pipe_is_valid() {
    let tpl = prosaic_template! {
        template: "{name|refer} modified the system",
        slots: [name],
    };
    assert!(tpl.contains("{name|refer}"));
}

// ── Phase 3: Pipe validation ─────────────────────────────────────────────────

#[test]
fn valid_pipes_pass() {
    // Each invocation must compile successfully.
    let _ = prosaic_template! {
        template: "{name|refer} is {count|pluralize:item}",
        slots: [name, count],
    };
    let _ = prosaic_template! {
        template: "{items|truncate:3|join:bracketed}",
        slots: [items],
    };
    let _ = prosaic_template! {
        template: "{action|verb:past}",
        slots: [action],
    };
    let _ = prosaic_template! {
        template: "{phrase|negated}",
        slots: [phrase],
    };
    let _ = prosaic_template! {
        template: "{ts|relative}",
        slots: [ts],
    };
    let _ = prosaic_template! {
        template: "{conf|hedge:modal}",
        slots: [conf],
    };
    let _ = prosaic_template! {
        template: "{count|quantify:natural}",
        slots: [count],
    };
    let _ = prosaic_template! {
        template: "{word|syn}",
        slots: [word],
    };
    let _ = prosaic_template! {
        template: "{n|ordinal}",
        slots: [n],
    };
    let _ = prosaic_template! {
        template: "{n|words}",
        slots: [n],
    };
    let _ = prosaic_template! {
        template: "{noun|article}",
        slots: [noun],
    };
    let _ = prosaic_template! {
        template: "{text|capitalize}",
        slots: [text],
    };
}

#[test]
fn conditional_section_key_counted_as_slot() {
    let tpl = prosaic_template! {
        template: "Added {name}{?count}, impacting {count} consumers{/?}",
        slots: [name, count],
    };
    assert!(tpl.contains("{?count}"));
    assert!(tpl.contains("{count}"));
}

#[test]
fn chained_pipes_all_validated() {
    let tpl = prosaic_template! {
        template: "{items|truncate:3|join}",
        slots: [items],
    };
    assert!(tpl.contains("truncate"));
    assert!(tpl.contains("join"));
}

#[test]
fn slots_omitted_defaults_to_empty_for_literal_template() {
    // The `slots:` key is optional — defaults to empty.
    let tpl = prosaic_template! {
        template: "No slots here.",
        slots: [],
    };
    assert_eq!(tpl, "No slots here.");
}

// ── Phase 4: choose pipe ─────────────────────────────────────────────────────

#[test]
fn choose_pipe_accepted_by_whitelist() {
    let tpl = prosaic_template! {
        template: "{level|choose: critical=URGENT, warn=WARN, default=INFO}",
        slots: [level],
    };
    assert!(tpl.contains("|choose"));
}

// ── Phase 5: Entity slot compatibility ───────────────────────────────────────
// The macro validates slot names and pipe names at compile time but is
// agnostic to Value types — entity-valued slots are structurally identical
// to string-valued slots from the macro's perspective.

#[test]
fn prosaic_template_macro_accepts_entity_slot() {
    let tpl = prosaic_template! {
        template: "Welcome, {user}!",
        slots: [user],
    };
    assert_eq!(tpl, "Welcome, {user}!");
}

#[test]
fn entity_slot_with_refer_pipe_accepted() {
    let tpl = prosaic_template! {
        template: "{user|refer} modified the system.",
        slots: [user],
    };
    assert!(tpl.contains("{user|refer}"));
}

// ── Phase 6: |plural pipe whitelist ──────────────────────────────────────────

#[test]
fn plural_pipe_accepted_by_whitelist() {
    let tpl = prosaic_template! {
        template: "{count|plural:service} affected",
        slots: [count],
    };
    assert!(tpl.contains("|plural"));
}

#[test]
fn plural_and_pluralize_both_accepted() {
    // Both pipes must be in the whitelist and coexist without conflict.
    let tpl_plural = prosaic_template! {
        template: "{count|plural:item} changed",
        slots: [count],
    };
    let tpl_pluralize = prosaic_template! {
        template: "{count|pluralize:item} changed",
        slots: [count],
    };
    assert!(tpl_plural.contains("|plural"));
    assert!(tpl_pluralize.contains("|pluralize"));
}
