//! Integration tests for `Engine::register_template_with_schema`.

use prosaic_core::{Engine, ProsaicError};
use prosaic_derive::IntoContext;
use prosaic_grammar_en::English;

#[derive(IntoContext)]
#[allow(dead_code)]
struct GoodCtx {
    count: i64,
}

#[derive(IntoContext)]
#[allow(dead_code)]
struct BadCtx {
    count: String,
}

#[test]
fn register_template_with_schema_accepts_matching_schema() {
    let mut engine = Engine::new(English::new());
    engine
        .register_template_with_schema::<GoodCtx>("ok", "{count|pluralize:item}")
        .unwrap();
}

#[test]
fn register_template_with_schema_rejects_type_mismatch() {
    let mut engine = Engine::new(English::new());
    let err = engine
        .register_template_with_schema::<BadCtx>("bad", "{count|pluralize:item}")
        .unwrap_err();
    match err {
        ProsaicError::TemplateParseError { reason, .. } => {
            assert!(reason.contains("count"), "reason: {reason}");
            assert!(
                reason.contains("Number") || reason.contains("String"),
                "reason: {reason}"
            );
        }
        other => panic!("expected TemplateParseError, got {other:?}"),
    }
}

#[test]
fn register_template_with_schema_rejects_missing_slot() {
    let mut engine = Engine::new(English::new());
    // GoodCtx has `count` but template uses `unknown_slot`.
    let err = engine
        .register_template_with_schema::<GoodCtx>("missing", "{unknown_slot|pluralize:item}")
        .unwrap_err();
    assert!(matches!(err, ProsaicError::TemplateParseError { .. }));
}
