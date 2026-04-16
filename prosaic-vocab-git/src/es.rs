//! Spanish-language git-activity templates.
//!
//! This module holds the concrete template strings for the `es` (Spanish)
//! locale. It is a sibling to `en.rs` and exposes the same `register`
//! signature, making locale-aware dispatch straightforward without touching
//! callers.
//!
//! Translations use idiomatic developer Spanish. Loanwords ("commit",
//! "pull request", "branch") are used where natural in tech contexts.

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full Spanish git-activity vocabulary into an engine.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    register_commit(engine)?;
    register_pr_opened(engine)?;
    register_pr_merged(engine)?;
    register_pr_closed(engine)?;
    register_issue_opened(engine)?;
    register_issue_closed(engine)?;
    register_review_approved(engine)?;
    register_review_changes_requested(engine)?;
    register_release(engine)?;
    Ok(())
}

fn register_commit(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: commits menores, pocos cambios
    engine.register_template_at(
        "git.commit",
        "{author} envió un cambio menor",
        Salience::Low,
    )?;
    engine.register_template_at(
        "git.commit",
        "{author} realizó un ajuste",
        Salience::Low,
    )?;

    // Medium: commit estándar con conteo de archivos
    engine.register_template(
        "git.commit",
        "{author} hizo un commit con {files_changed} \
         {files_changed|pluralize:archivo}{?message}: \"{message}\"{/?}",
    )?;
    engine.register_template(
        "git.commit",
        "{author} envió un commit que toca {files_changed} \
         {files_changed|pluralize:archivo}{?message} \u{2014} {message}{/?}",
    )?;

    // High: commits grandes con mucho cambio
    engine.register_template_at(
        "git.commit",
        "{author} realizó un commit de gran envergadura abarcando {files_changed} \
         {files_changed|pluralize:archivo}, añadiendo {additions} {additions|pluralize:línea} \
         y eliminando {deletions} {deletions|pluralize:línea}{?message}. La descripción \
         del cambio indica: \"{message}\"{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_pr_opened(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.pr_opened",
        "{author} abrió el PR #{number}: \"{title}\"",
    )?;
    engine.register_template(
        "git.pr_opened",
        "El PR #{number} fue abierto por {author} \u{2014} \"{title}\"",
    )?;
    engine.register_template_at(
        "git.pr_opened",
        "Un nuevo pull request \u{2014} #{number}, \"{title}\" \u{2014} \
         fue abierto por {author}",
        Salience::High,
    )?;
    Ok(())
}

fn register_pr_merged(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.pr_merged",
        "El PR #{number} \"{title}\" fue fusionado por {merger}",
    )?;
    engine.register_template(
        "git.pr_merged",
        "{merger} fusionó el PR #{number} (\"{title}\"){?author}, \
         originalmente creado por {author}{/?}",
    )?;
    engine.register_template_at(
        "git.pr_merged",
        "El PR #{number} ha sido fusionado",
        Salience::Low,
    )?;
    Ok(())
}

fn register_pr_closed(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.pr_closed",
        "El PR #{number} \"{title}\" fue cerrado{?reason} \u{2014} {reason}{/?}",
    )?;
    engine.register_template_at(
        "git.pr_closed",
        "El PR #{number} fue cerrado sin fusionar",
        Salience::Low,
    )?;
    Ok(())
}

fn register_issue_opened(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.issue_opened",
        "{author} reportó la incidencia #{number}: \"{title}\"",
    )?;
    engine.register_template(
        "git.issue_opened",
        "La incidencia #{number} \"{title}\" fue abierta por {author}",
    )?;
    Ok(())
}

fn register_issue_closed(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.issue_closed",
        "La incidencia #{number} \"{title}\" fue cerrada{?reason} como {reason}{/?}",
    )?;
    engine.register_template_at(
        "git.issue_closed",
        "La incidencia #{number} fue resuelta",
        Salience::Low,
    )?;
    Ok(())
}

fn register_review_approved(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.review_approved",
        "{reviewer} aprobó el PR #{pr_number}",
    )?;
    engine.register_template(
        "git.review_approved",
        "El PR #{pr_number} recibió la aprobación de {reviewer}",
    )?;
    Ok(())
}

fn register_review_changes_requested(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.review_changes_requested",
        "{reviewer} solicitó cambios en el PR #{pr_number}{?comment_count} \
         ({comment_count} {comment_count|pluralize:comentario}){/?}",
    )?;
    engine.register_template(
        "git.review_changes_requested",
        "El PR #{pr_number} necesita revisiones \u{2014} {reviewer} dejó \
         {comment_count} {comment_count|pluralize:comentario}",
    )?;
    Ok(())
}

fn register_release(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "git.release",
        "La versión {version} fue etiquetada como {tag}{?changes_count}, \
         abarcando {changes_count} {changes_count|pluralize:cambio}{/?}",
    )?;
    engine.register_template_at(
        "git.release",
        "Un nuevo lanzamiento \u{2014} {version}, etiquetado como {tag} \u{2014} \
         fue publicado con {changes_count} {changes_count|pluralize:cambio} destacado",
        Salience::High,
    )?;
    engine.register_template_at(
        "git.release",
        "{version} fue lanzado",
        Salience::Low,
    )?;
    Ok(())
}
