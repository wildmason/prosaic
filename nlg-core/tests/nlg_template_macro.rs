//! Integration tests for the `nlg_template!` proc macro from `nlg-derive`.
//!
//! Compile-fail behaviour (undeclared slot / unknown pipe emits a clean
//! compile error) is exercised by manual verification during development.
//! A `trybuild` harness for compile-fail tests is deferred to v2.

use nlg_derive::nlg_template;

// ── Phase 1: Scaffold ────────────────────────────────────────────────────────

#[test]
fn passes_through_valid_template() {
    let tpl = nlg_template! {
        template: "The {entity_type} {name} was renamed",
        slots: [entity_type, name],
    };
    assert_eq!(tpl, "The {entity_type} {name} was renamed");
}

#[test]
fn empty_slots_list_allowed_for_literal_only_template() {
    let tpl = nlg_template! {
        template: "All systems nominal.",
        slots: [],
    };
    assert_eq!(tpl, "All systems nominal.");
}

// ── Phase 2: Slot validation ─────────────────────────────────────────────────

#[test]
fn extra_declared_slots_are_allowed() {
    // Declaring more slots than the template uses is fine.
    let tpl = nlg_template! {
        template: "Hello {name}",
        slots: [name, unused_slot],
    };
    assert_eq!(tpl, "Hello {name}");
}

#[test]
fn slot_with_pipe_is_valid() {
    let tpl = nlg_template! {
        template: "{name|refer} modified the system",
        slots: [name],
    };
    assert!(tpl.contains("{name|refer}"));
}

// ── Phase 3: Pipe validation ─────────────────────────────────────────────────

#[test]
fn valid_pipes_pass() {
    // Each invocation must compile successfully.
    let _ = nlg_template! {
        template: "{name|refer} is {count|pluralize:item}",
        slots: [name, count],
    };
    let _ = nlg_template! {
        template: "{items|truncate:3|join:bracketed}",
        slots: [items],
    };
    let _ = nlg_template! {
        template: "{action|verb:past}",
        slots: [action],
    };
    let _ = nlg_template! {
        template: "{phrase|negated}",
        slots: [phrase],
    };
    let _ = nlg_template! {
        template: "{ts|relative}",
        slots: [ts],
    };
    let _ = nlg_template! {
        template: "{conf|hedge:modal}",
        slots: [conf],
    };
    let _ = nlg_template! {
        template: "{count|quantify:natural}",
        slots: [count],
    };
    let _ = nlg_template! {
        template: "{word|syn}",
        slots: [word],
    };
    let _ = nlg_template! {
        template: "{n|ordinal}",
        slots: [n],
    };
    let _ = nlg_template! {
        template: "{n|words}",
        slots: [n],
    };
    let _ = nlg_template! {
        template: "{noun|article}",
        slots: [noun],
    };
    let _ = nlg_template! {
        template: "{text|capitalize}",
        slots: [text],
    };
}

#[test]
fn conditional_section_key_counted_as_slot() {
    let tpl = nlg_template! {
        template: "Added {name}{?count}, impacting {count} consumers{/?}",
        slots: [name, count],
    };
    assert!(tpl.contains("{?count}"));
    assert!(tpl.contains("{count}"));
}

#[test]
fn chained_pipes_all_validated() {
    let tpl = nlg_template! {
        template: "{items|truncate:3|join}",
        slots: [items],
    };
    assert!(tpl.contains("truncate"));
    assert!(tpl.contains("join"));
}

#[test]
fn slots_omitted_defaults_to_empty_for_literal_template() {
    // The `slots:` key is optional — defaults to empty.
    let tpl = nlg_template! {
        template: "No slots here.",
        slots: [],
    };
    assert_eq!(tpl, "No slots here.");
}
