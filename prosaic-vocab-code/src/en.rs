//! English-language code-analysis templates.
//!
//! This module holds the concrete template strings for the `en` (English)
//! locale. When additional locales are added (e.g. `es`, `de`), they will
//! each live in a sibling module and expose the same `register` signature,
//! making locale-aware dispatch straightforward without touching callers.

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full English code-analysis vocabulary into an engine.
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
    // Low: terse, drops impact details
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} was renamed to {new_name}",
        Salience::Low,
    )?;
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} is now called {new_name}",
        Salience::Low,
    )?;

    // Medium: default, includes impact clause
    engine.register_template(
        "code.renamed",
        "{old_name|refer} was renamed to {new_name}{?consumer_count}, \
         which impacts {consumer_count} direct {consumer_count|pluralize:consumer}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.renamed",
        "{old_name|refer} has been renamed to {new_name}{?consumer_count}, \
         affecting {consumer_count} {consumer_count|pluralize:dependent}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.renamed",
        "{old_name|refer} is now called {new_name}{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:consumer} affected{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;

    // High: elaborative, emphasizes significance.
    // Uses plain `join:bracketed` so the template's own literal prefix
    // ("including", "notably") isn't duplicated by an auto-selected list style.
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} has been renamed to {new_name} \u{2014} a significant change \
         rippling through {consumer_count} direct {consumer_count|pluralize:consumer}{?consumers}, \
         including {consumers|truncate:5|join:bracketed}{/?}",
        Salience::High,
    )?;
    engine.register_template_at(
        "code.renamed",
        "{old_name|refer} was renamed to {new_name}. This change affects a substantial \
         {consumer_count} {consumer_count|pluralize:dependent}{?consumers} \u{2014} notably \
         {consumers|truncate:5|join:bracketed}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_delete_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low
    engine.register_template_at(
        "code.deleted",
        "{name|refer} was removed",
        Salience::Low,
    )?;
    engine.register_template_at(
        "code.deleted",
        "{name|refer} has been deleted",
        Salience::Low,
    )?;

    // Medium
    engine.register_template(
        "code.deleted",
        "{name|refer} was removed{?consumer_count}, \
         impacting {consumer_count} {consumer_count|pluralize:dependent}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.deleted",
        "{name|refer} has been deleted{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:reference} to update{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    engine.register_template(
        "code.deleted",
        "{name|refer} no longer exists{?consumer_count}, \
         breaking {consumer_count} {consumer_count|pluralize:consumer}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;

    // High
    engine.register_template_at(
        "code.deleted",
        "{name|refer} has been removed entirely \u{2014} a breaking change affecting \
         {consumer_count} {consumer_count|pluralize:consumer}{?consumers} including \
         {consumers|truncate:5|join:bracketed}{/?}. All {consumer_count|pluralize:reference} \
         will need migration.",
        Salience::High,
    )?;

    Ok(())
}

fn register_add_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "code.added",
        "A new {entity_type} {name} was added in {location}",
    )?;
    engine.register_template(
        "code.added",
        "The {entity_type} {name} was introduced in {location}",
    )?;
    engine.register_template(
        "code.added",
        "{name} \u{2014} a new {entity_type} \u{2014} was created in {location}",
    )?;
    Ok(())
}

fn register_modify_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low
    engine.register_template_at(
        "code.modified",
        "{name|refer} was modified",
        Salience::Low,
    )?;
    engine.register_template_at(
        "code.modified",
        "{name|refer} has been updated",
        Salience::Low,
    )?;

    // Medium
    engine.register_template(
        "code.modified",
        "{name|refer} was modified{?consumer_count}, \
         which may affect {consumer_count} {consumer_count|pluralize:consumer}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.modified",
        "{name|refer} has been updated{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:consumer} may need review{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    engine.register_template(
        "code.modified",
        "Changes to {name|refer}{?consumer_count} affect \
         {consumer_count} {consumer_count|pluralize:dependent}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;

    // High
    engine.register_template_at(
        "code.modified",
        "{name|refer} has been substantially modified, with downstream impact \
         across {consumer_count} {consumer_count|pluralize:consumer}{?consumers} \
         including {consumers|truncate:5|join:bracketed}{/?}. Thorough review is \
         recommended.",
        Salience::High,
    )?;

    Ok(())
}

fn register_move_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "code.moved",
        "{name|refer} was moved from {old_location} to {new_location}{?consumer_count}, \
         requiring import updates in {consumer_count} {consumer_count|pluralize:file}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.moved",
        "{name|refer} has been relocated to {new_location}{?consumer_count} \
         ({consumer_count} {consumer_count|pluralize:import} to update{?consumers}: \
         {consumers|truncate:3|join}{/?}){/?}",
    )?;
    Ok(())
}

fn register_signature_templates(engine: &mut Engine) -> Result<(), ProsaicError> {
    engine.register_template(
        "code.signature_changed",
        "The signature of {name|refer} was changed{?consumer_count}, \
         requiring updates in {consumer_count} {consumer_count|pluralize:caller}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    engine.register_template(
        "code.signature_changed",
        "{name|refer} has a new signature{?consumer_count}, \
         impacting {consumer_count} call {consumer_count|pluralize:site}{?consumers} \
         {consumers|truncate:3|join}{/?}{/?}",
    )?;
    Ok(())
}
