//! Integration tests for the `proportion` pipe.
//!
//! Exercises the full render path so the pipe arg parsing
//! (`proportion:total_key[:noun]`) and the context-key lookup for the
//! denominator are covered alongside the phrasing logic.

use prosaic_core::{Context, Engine, Session, Strictness, Value, Variation};
use prosaic_grammar_en::English;

fn engine() -> Engine {
    Engine::new(English::new())
        .strictness(Strictness::Strict)
        .variation(Variation::Fixed)
}

fn ctx_pair(matching: i64, total: i64) -> Context {
    let mut c = Context::new();
    c.insert("matching", Value::Number(matching));
    c.insert("total", Value::Number(total));
    c
}

// ── Crucible-style end-to-end: proportion embedded in a full sentence ────

#[test]
fn crucible_use_case_both_collapse() {
    // Realistic template: the proportion phrase is embedded inside a
    // sentence that already starts with a capital and ends with a period.
    let mut engine = engine();
    engine
        .register_template(
            "summary",
            "The bulk of this changeset lives in src, \
             with {matching|proportion:total:modified file} belonging to that module.",
        )
        .unwrap();
    let mut session = Session::new();
    let out = engine
        .render(&mut session, "summary", &ctx_pair(2, 2))
        .unwrap();
    assert_eq!(
        out,
        "The bulk of this changeset lives in src, with both modified files \
         belonging to that module."
    );
}

#[test]
fn crucible_use_case_all_n_collapse() {
    let mut engine = engine();
    engine
        .register_template(
            "summary",
            "The bulk of this changeset lives in src, \
             with {matching|proportion:total:modified file} belonging to that module.",
        )
        .unwrap();
    let mut session = Session::new();
    let out = engine
        .render(&mut session, "summary", &ctx_pair(13, 13))
        .unwrap();
    assert_eq!(
        out,
        "The bulk of this changeset lives in src, with all 13 modified files \
         belonging to that module."
    );
}

#[test]
fn crucible_use_case_partial_keeps_n_of_t() {
    let mut engine = engine();
    engine
        .register_template(
            "summary",
            "The bulk of this changeset lives in src, \
             with {matching|proportion:total:modified file} belonging to that module.",
        )
        .unwrap();
    let mut session = Session::new();
    let out = engine
        .render(&mut session, "summary", &ctx_pair(3, 13))
        .unwrap();
    assert_eq!(
        out,
        "The bulk of this changeset lives in src, with 3 of 13 modified files \
         belonging to that module."
    );
}

// ── Bare-pipe verification: pipe output before engine post-processing ────

#[test]
fn bare_one_of_one_uses_singular_noun() {
    let mut engine = engine();
    engine
        .register_template("files", "{matching|proportion:total:modified file}")
        .unwrap();
    let mut session = Session::new();
    let out = engine.render(&mut session, "files", &ctx_pair(1, 1)).unwrap();
    assert_eq!(out, "the only modified file");
}

#[test]
fn bare_zero_zero_with_noun_reads_no_plural() {
    let mut engine = engine();
    engine
        .register_template("files", "{matching|proportion:total:modified file}")
        .unwrap();
    let mut session = Session::new();
    let out = engine.render(&mut session, "files", &ctx_pair(0, 0)).unwrap();
    assert_eq!(out, "no modified files");
}

#[test]
fn bare_zero_of_n_with_noun_reads_none_of_the_n() {
    let mut engine = engine();
    engine
        .register_template("files", "{matching|proportion:total:modified file}")
        .unwrap();
    let mut session = Session::new();
    let out = engine.render(&mut session, "files", &ctx_pair(0, 5)).unwrap();
    assert_eq!(out, "none of the 5 modified files");
}

// ── Noun-less form ──────────────────────────────────────────────────────

#[test]
fn bare_both_collapse_no_noun() {
    let mut engine = engine();
    engine
        .register_template("p", "{matching|proportion:total}")
        .unwrap();
    let mut session = Session::new();
    let out = engine.render(&mut session, "p", &ctx_pair(2, 2)).unwrap();
    assert_eq!(out, "both");
}

#[test]
fn bare_all_n_no_noun() {
    let mut engine = engine();
    engine
        .register_template("p", "{matching|proportion:total}")
        .unwrap();
    let mut session = Session::new();
    let out = engine.render(&mut session, "p", &ctx_pair(7, 7)).unwrap();
    assert_eq!(out, "all 7");
}

#[test]
fn bare_partial_no_noun() {
    let mut engine = engine();
    engine
        .register_template("p", "{matching|proportion:total}")
        .unwrap();
    let mut session = Session::new();
    let out = engine.render(&mut session, "p", &ctx_pair(3, 7)).unwrap();
    assert_eq!(out, "3 of 7");
}

// ── Error paths ──────────────────────────────────────────────────────────

#[test]
fn missing_total_context_key_errors_in_strict_mode() {
    let mut engine = engine();
    engine
        .register_template("p", "{matching|proportion:total:file}")
        .unwrap();
    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("matching", Value::Number(2));
    // total is intentionally missing
    let err = engine.render(&mut session, "p", &ctx).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("total") && msg.contains("proportion"),
        "expected error mentioning the missing total key, got: {msg}"
    );
}

#[test]
fn non_numeric_total_errors() {
    let mut engine = engine();
    engine
        .register_template("p", "{matching|proportion:total:file}")
        .unwrap();
    let mut session = Session::new();
    let mut ctx = Context::new();
    ctx.insert("matching", Value::Number(2));
    ctx.insert("total", Value::String("five".into()));
    let err = engine.render(&mut session, "p", &ctx).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("number") || msg.contains("numeric"),
        "expected error about non-numeric total, got: {msg}"
    );
}

#[test]
fn pipe_with_no_argument_errors() {
    let mut engine = engine();
    engine
        .register_template("p", "{matching|proportion}")
        .unwrap();
    let mut session = Session::new();
    let err = engine
        .render(&mut session, "p", &ctx_pair(2, 2))
        .unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("proportion"),
        "expected error mentioning proportion pipe, got: {msg}"
    );
}
