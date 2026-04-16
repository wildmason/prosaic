//! Spanish-language PR-narrative templates.
//!
//! This module holds the concrete template strings for the `es` (Spanish)
//! locale. It is a sibling to `en.rs` and exposes the same `register`
//! signature, making locale-aware dispatch straightforward without touching
//! callers.
//!
//! Translations use idiomatic developer Spanish. "Pull request" is kept
//! as a loanword since it is universally understood in Spanish tech contexts.

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full Spanish PR-narrative vocabulary into an engine.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    register_summary(engine)?;
    register_review_state(engine)?;
    register_scope(engine)?;
    register_diff_stats(engine)?;
    register_age(engine)?;
    register_merge_readiness(engine)?;
    register_ci_status(engine)?;
    register_related_prs(engine)?;
    Ok(())
}

fn register_summary(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: solo referencia al título
    engine.register_template_at("pr.summary", "PR #{number}: {title}", Salience::Low)?;

    // Medium: añade el autor
    engine.register_template(
        "pr.summary",
        "PR #{number} \u{2014} \u{201c}{title}\u{201d} por {author}",
    )?;

    // High: detalle completo con conteos de commits y archivos
    engine.register_template_at(
        "pr.summary",
        "PR #{number} por {author}: \u{201c}{title}\u{201d} \u{2014} \
         {commit_count} {commit_count|pluralize:commit} en \
         {files_changed} {files_changed|pluralize:archivo}",
        Salience::High,
    )?;

    Ok(())
}

fn register_review_state(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: solo conteo de aprobaciones
    engine.register_template_at(
        "pr.review_state",
        "El PR #{number} tiene {approvals} {approvals|pluralize:aprobación}",
        Salience::Low,
    )?;

    // Medium: panorama completo de la revisión
    engine.register_template(
        "pr.review_state",
        "El PR #{number} tiene {approvals} {approvals|pluralize:aprobación}\
         {?requested_changes}, {requested_changes} \
         {requested_changes|pluralize:solicitud} de cambios{/?}\
         {?pending}, y {pending} {pending|pluralize:revisión} pendiente{/?}",
    )?;

    Ok(())
}

fn register_scope(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: conteo conciso de áreas
    engine.register_template_at(
        "pr.scope",
        "El PR #{number} toca {areas|truncate:3|join}",
        Salience::Low,
    )?;

    // Medium: enmarcado como declaración de alcance
    engine.register_template("pr.scope", "El PR #{number} abarca {areas|truncate:3|join}")?;

    Ok(())
}

fn register_diff_stats(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low only — las estadísticas del diff son detalle complementario
    engine.register_template_at(
        "pr.diff_stats",
        "PR #{number}: +{insertions} \u{2212}{deletions} {deletions|pluralize:línea}",
        Salience::Low,
    )?;

    engine.register_template(
        "pr.diff_stats",
        "El PR #{number} añade {insertions} {insertions|pluralize:línea} \
         y elimina {deletions} {deletions|pluralize:línea}",
    )?;

    Ok(())
}

fn register_age(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: antigüedad simple
    engine.register_template_at(
        "pr.age",
        "El PR #{number} lleva abierto {days_open} {days_open|pluralize:día}",
        Salience::Low,
    )?;

    // Medium: antigüedad con indicador de abandono via |choose
    engine.register_template(
        "pr.age",
        "El PR #{number} lleva abierto {days_open} \
         {days_open|pluralize:día}{stale|choose: 1= y está desactualizado, default=}",
    )?;

    Ok(())
}

fn register_merge_readiness(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Medium: listo/no listo con bloqueadores opcionales
    engine.register_template(
        "pr.merge_readiness",
        "El PR #{number} {ready|choose: 1=está listo para fusionar, default=aún no está listo}\
         {?blockers}, bloqueado por {blockers|truncate:3|join}{/?}",
    )?;

    // High: fraseo más enfático
    engine.register_template_at(
        "pr.merge_readiness",
        "PR #{number} \u{2014} {ready|choose: 1=este PR está listo para fusionar, \
         default=este PR aún no está listo}{?blockers}. \
         Bloqueadores: {blockers|truncate:3|join}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_ci_status(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: solo conteo de verificaciones exitosas
    engine.register_template_at(
        "pr.ci_status",
        "PR #{number}: {passing} {passing|pluralize:verificación} CI exitosa",
        Salience::Low,
    )?;

    // Medium: exitosas más fallidas
    engine.register_template(
        "pr.ci_status",
        "El PR #{number} tiene {passing} {passing|pluralize:verificación} exitosa\
         {?failing} y {failing} {failing|pluralize:verificación} fallida{/?}",
    )?;

    Ok(())
}

fn register_related_prs(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Medium only — el contexto relacional es inherentemente de verbosidad media
    engine.register_template(
        "pr.related_prs",
        "El PR #{number}{?depends_on} depende de {depends_on|truncate:3|join}{/?}\
         {?blocks} y bloquea {blocks|truncate:3|join}{/?}",
    )?;

    Ok(())
}
