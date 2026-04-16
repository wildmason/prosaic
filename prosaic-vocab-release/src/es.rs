//! Spanish-language release-note templates.
//!
//! This module holds the concrete template strings for the `es` (Spanish)
//! locale. It is a sibling to `en.rs` and exposes the same `register`
//! signature, making locale-aware dispatch straightforward without touching
//! callers.
//!
//! Translations use idiomatic developer Spanish. Technical loanwords are
//! preferred where natural in Spanish tech communities.

use prosaic_core::{Engine, ProsaicError, Salience};

/// Register the full Spanish release-note vocabulary into an engine.
pub fn register(engine: &mut Engine) -> Result<(), ProsaicError> {
    register_tagged(engine)?;
    register_feature_added(engine)?;
    register_breaking_change(engine)?;
    register_bugfix(engine)?;
    register_security_fix(engine)?;
    register_deprecation(engine)?;
    register_dependency_update(engine)?;
    register_contributor_summary(engine)?;
    register_stats(engine)?;
    register_summary(engine)?;
    Ok(())
}

fn register_tagged(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: anuncio básico de etiqueta
    engine.register_template_at(
        "release.tagged",
        "La versión {version} fue etiquetada",
        Salience::Low,
    )?;

    // Medium: incluye título opcional
    engine.register_template(
        "release.tagged",
        "La versión {version}{?title} \u{2014} {title}{/?} ya está etiquetada",
    )?;

    // High: detalle completo con sha y fecha opcionales
    engine.register_template_at(
        "release.tagged",
        "La versión {version}{?title} (\u{201c}{title}\u{201d}){/?} ha sido etiquetada \
         y publicada{?date} el {date}{/?}{?commit_sha} en el commit {commit_sha}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_feature_added(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: conciso
    engine.register_template_at("release.feature_added", "Se agregó {name}", Salience::Low)?;

    // Medium: nombra la funcionalidad, muestra descripción si está presente
    engine.register_template(
        "release.feature_added",
        "Nueva funcionalidad: {name}{?description} \u{2014} {description}{/?}",
    )?;

    // High: anuncio elaborado que invita a la adopción
    engine.register_template_at(
        "release.feature_added",
        "Esta versión introduce {name}{?description}. {description}{/?} \
         Los usuarios existentes pueden querer probarlo.",
        Salience::High,
    )?;

    Ok(())
}

fn register_breaking_change(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Los cambios disruptivos son siempre de alta importancia
    engine.register_template_at(
        "release.breaking_change",
        "{name} introduce un cambio disruptivo{?migration_path}; \
         migración: {migration_path}{/?}",
        Salience::High,
    )?;
    engine.register_template_at(
        "release.breaking_change",
        "Cambio disruptivo: {name}{?migration_path}. Para migrar: {migration_path}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_bugfix(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: solo la descripción
    engine.register_template_at("release.bugfix", "Corregido: {description}", Salience::Low)?;

    // Medium: enlaza con el número de incidencia cuando está disponible
    engine.register_template(
        "release.bugfix",
        "Corrección de error: {description}{?issue_number} (incidencia #{issue_number}){/?}",
    )?;

    Ok(())
}

fn register_security_fix(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Las correcciones de seguridad son siempre de alta importancia
    engine.register_template_at(
        "release.security_fix",
        "Corrección de seguridad: {description}{?severity} (gravedad: {severity}){/?}\
         {?cve} \u{2014} {cve}{/?}",
        Salience::High,
    )?;
    engine.register_template_at(
        "release.security_fix",
        "Se ha corregido una vulnerabilidad de seguridad: {description}{?cve} ({cve}){/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_deprecation(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Medium: nombra el elemento deprecado
    engine.register_template(
        "release.deprecation",
        "{name} está deprecado{?replacement} en favor de {replacement}{/?}\
         {?removal_version} y será eliminado en {removal_version}{/?}",
    )?;

    // High: advertencia más enfática
    engine.register_template_at(
        "release.deprecation",
        "Aviso de deprecación: {name} ha quedado deprecado{?replacement} \
         \u{2014} use {replacement} en su lugar{/?}{?removal_version}. \
         Será eliminado en {removal_version}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_dependency_update(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: actualización de versión concisa
    engine.register_template_at(
        "release.dependency_update",
        "{name} actualizado de {from_version} a {to_version}",
        Salience::Low,
    )?;

    // Medium: añade motivo opcional
    engine.register_template(
        "release.dependency_update",
        "La dependencia {name} fue actualizada de {from_version} a {to_version}\
         {?reason} \u{2014} {reason}{/?}",
    )?;

    Ok(())
}

fn register_contributor_summary(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: solo el conteo
    engine.register_template_at(
        "release.contributor_summary",
        "{count} {count|pluralize:colaborador} contribuyó a esta versión",
        Salience::Low,
    )?;

    // Medium: nombra a los principales colaboradores si están disponibles
    engine.register_template(
        "release.contributor_summary",
        "Esta versión contó con {count} {count|pluralize:colaborador}\
         {?top_contributors}: {top_contributors|truncate:3|join}{/?}",
    )?;

    // High: fraseo orientado al reconocimiento
    engine.register_template_at(
        "release.contributor_summary",
        "Gracias a los {count} {count|pluralize:colaborador} que hicieron \
         posible esta versión{?top_contributors}, incluyendo {top_contributors|truncate:3|join}{/?}",
        Salience::High,
    )?;

    Ok(())
}

fn register_stats(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Low: solo conteo de commits
    engine.register_template_at(
        "release.stats",
        "{commits} {commits|pluralize:commit} desde la última versión",
        Salience::Low,
    )?;

    // Medium: estadísticas completas del diff
    engine.register_template(
        "release.stats",
        "{commits} {commits|pluralize:commit} en {files_changed} \
         {files_changed|pluralize:archivo}, añadiendo {insertions} \
         {insertions|pluralize:línea} y eliminando {deletions} {deletions|pluralize:línea}",
    )?;

    Ok(())
}

fn register_summary(engine: &mut Engine) -> Result<(), ProsaicError> {
    // Medium: titular conciso
    engine.register_template("release.summary", "{version}: {headline}")?;

    // High: anuncio elaborado de la versión
    engine.register_template_at(
        "release.summary",
        "La versión {version} ya está disponible. {headline}",
        Salience::High,
    )?;

    Ok(())
}
