//! Spanish-language code-analysis templates.
//!
//! This module holds the concrete template strings for the `es` (Spanish)
//! locale. It is a sibling to `en.rs` and exposes the same `register`
//! signature, making locale-aware dispatch straightforward without touching
//! callers.
//!
//! Translations use idiomatic developer Spanish. Loanwords ("commit",
//! "pull request") are kept where they are the natural choice. Accents
//! are correct UTF-8 throughout.

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full Spanish code-analysis vocabulary into an engine.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    register_rename_templates(engine)?;
    register_delete_templates(engine)?;
    register_add_templates(engine)?;
    register_modify_templates(engine)?;
    register_move_templates(engine)?;
    register_signature_templates(engine)?;
    Ok(())
}

fn register_rename_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: conciso, sin detalle de impacto
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} fue renombrado a {new_name}",
        Salience::Low,
    )?;
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} ahora se llama {new_name}",
        Salience::Low,
    )?;

    // Medium: incluye cláusula de impacto
    engine.register_template(
        "code.renamed",
        "{old_name|refer} fue renombrado a {new_name}{?consumer_count}, \
         lo que afecta a {consumer_count} {consumer_count|pluralize:consumidor} \
         directo{?consumers} {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.renamed",
        "{old_name|refer} ha sido renombrado a {new_name}{?consumer_count}, \
         afectando a {consumer_count} {consumer_count|pluralize:dependiente}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.renamed",
        "{old_name|refer} ahora se llama {new_name}{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:consumidor} afectado{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;

    // High: elaborado, enfatiza la magnitud del cambio
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} ha sido renombrado a {new_name} \u{2014} un cambio significativo \
         que repercute en {consumer_count} {consumer_count|pluralize:consumidor} directo{?consumers}, \
         incluyendo {consumers|truncate:5|join:bracketed}{/?}",
        Salience::High,
    )?;
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} fue renombrado a {new_name}. Este cambio afecta a un total \
         de {consumer_count} {consumer_count|pluralize:dependiente}{?consumers} \u{2014} \
         particularmente {consumers|truncate:5|join:bracketed}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_delete_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low
    engine.register_template_at("code.deleted", "{name|refer} fue eliminado", Salience::Low)?;
    engine.register_template_at(
        "code.deleted",
        "{name|refer} ha sido eliminado",
        Salience::Low,
    )?;

    // Medium
    engine.register_template(
        "code.deleted",
        "{name|refer} fue eliminado{?consumer_count}, \
         afectando a {consumer_count} {consumer_count|pluralize:dependiente}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.deleted",
        "{name|refer} ha sido eliminado{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:referencia} por actualizar{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    engine.register_template(
        "code.deleted",
        "{name|refer} ya no existe{?consumer_count}, \
         dejando sin dependencia a {consumer_count} {consumer_count|pluralize:consumidor}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;

    // High
    engine.register_template_at(
        "code.deleted",
        "{name|refer} ha sido eliminado por completo \u{2014} un cambio disruptivo que afecta a \
         {consumer_count} {consumer_count|pluralize:consumidor}{?consumers} incluyendo \
         {consumers|truncate:5|join:bracketed}{/?}. Todas las {consumer_count|pluralize:referencia} \
         necesitarán migración.",
        Salience::High,
    )?;

    Ok(())
}

fn register_add_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "code.added",
        "Se agregó un nuevo {entity_type} {name} en {location}",
    )?;
    engine.register_template(
        "code.added",
        "El {entity_type} {name} fue introducido en {location}",
    )?;
    engine.register_template(
        "code.added",
        "{name} \u{2014} un nuevo {entity_type} \u{2014} fue creado en {location}",
    )?;
    Ok(())
}

fn register_modify_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low
    engine.register_template_at(
        "code.modified",
        "{name|refer} fue modificado",
        Salience::Low,
    )?;
    engine.register_template_at(
        "code.modified",
        "{name|refer} ha sido actualizado",
        Salience::Low,
    )?;

    // Medium
    engine.register_template(
        "code.modified",
        "{name|refer} fue modificado{?consumer_count}, \
         lo que podría afectar a {consumer_count} {consumer_count|pluralize:consumidor}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.modified",
        "{name|refer} ha sido actualizado{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:consumidor} podría necesitar revisión{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    engine.register_template(
        "code.modified",
        "Los cambios en {name|refer}{?consumer_count} afectan a \
         {consumer_count} {consumer_count|pluralize:dependiente}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;

    // High
    engine.register_template_at(
        "code.modified",
        "{name|refer} ha sido modificado de forma significativa, con impacto descendente \
         en {consumer_count} {consumer_count|pluralize:consumidor}{?consumers} \
         incluyendo {consumers|truncate:5|join:bracketed}{/?}. \
         Se recomienda una revisión exhaustiva.",
        Salience::High,
    )?;

    Ok(())
}

fn register_move_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "code.moved",
        "{name|refer} fue movido de {old_location} a {new_location}{?consumer_count}, \
         lo que requiere actualizar las importaciones en {consumer_count} \
         {consumer_count|pluralize:archivo}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.moved",
        "{name|refer} ha sido trasladado a {new_location}{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:importación} por actualizar{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    Ok(())
}

fn register_signature_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "code.signature_changed",
        "La firma de {name|refer} fue modificada{?consumer_count}, \
         lo que requiere cambios en {consumer_count} {consumer_count|pluralize:invocador}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.signature_changed",
        "{name|refer} tiene una nueva firma{?consumer_count}, \
         afectando a {consumer_count} {consumer_count|pluralize:invocador}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    Ok(())
}
