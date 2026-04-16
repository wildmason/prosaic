//! Spanish grammar layer for the Prosaic NLG engine.
//!
//! Minimal v1.5 scope: gender-aware articles, regular + common-irregular
//! pluralization, regular verb conjugation (present/simple-past/future/
//! past-participle/present-participle), gender agreement on passive
//! participles, gendered pronouns.
//!
//! Uses only pure Rust — no CLDR data. Expansion (subjunctive, imperfecto,
//! full irregular tables, dialectal variation) is planned for follow-up crates.

// TODO Phase 2+: full Language trait implementation
pub mod gender;

/// Spanish language grammar implementation.
#[derive(Debug, Clone, Default)]
pub struct Spanish;

impl Spanish {
    pub fn new() -> Self {
        Self
    }
}
