//! General-purpose natural language generation from structured data.
//!
//! Takes structured events and produces **natural-sounding** prose, not
//! just grammatically correct output. The engine tracks discourse state
//! across calls, so multiple renders flow together like human-written
//! prose — using pronouns, varying phrasing, matching verbosity to
//! impact, and structuring multi-paragraph narratives.
//!
//! English, Spanish, and German grammars ship out of the box via the
//! `prosaic-grammar-en`, `-es`, and `-de` sibling crates. Add more
//! languages by implementing the [`Language`] trait.
//!
//! # Quick start
//!
//! ```
//! use prosaic_core::{Context, Engine, Session, Strictness, Value, Variation};
//! use prosaic_grammar_en::English;
//!
//! let mut engine = Engine::new(English::new())
//!     .strictness(Strictness::Strict)
//!     .variation(Variation::Fixed);
//!
//! engine.register_template(
//!     "entity.renamed",
//!     "{old_name|refer} was renamed to {new_name}",
//! ).unwrap();
//!
//! let mut ctx = Context::new();
//! ctx.insert("entity_type", Value::String("class".into()));
//! ctx.insert("old_name", Value::String("Foo".into()));
//! ctx.insert("new_name", Value::String("Foobar".into()));
//!
//! let mut session = Session::new();
//! let sentence = engine.render(&mut session, "entity.renamed", &ctx).unwrap();
//! assert_eq!(sentence, "The class Foo was renamed to Foobar.");
//! ```
//!
//! # Type-aware template validation
//!
//! Context types that derive `IntoContext` also get a `HasProsaicSchema`
//! impl for free. Pair it with the `context:` argument of
//! `prosaic_template!` to validate slot types at compile time:
//!
//! ```no_run
//! use prosaic_core::{Engine, Session};
//! use prosaic_derive::{IntoContext, prosaic_template};
//! use prosaic_grammar_en::English;
//!
//! #[derive(IntoContext)]
//! struct RenameCtx {
//!     old_name: String,
//!     new_name: String,
//!     consumer_count: i64,
//! }
//!
//! // Compile error if `consumer_count` were declared as `String`:
//! let tpl = prosaic_template! {
//!     template: "{old_name} → {new_name} ({consumer_count|pluralize:consumer})",
//!     slots: [old_name, new_name, consumer_count],
//!     context: RenameCtx,
//! };
//!
//! let mut engine = Engine::new(English::new());
//! engine.register_template("rename", tpl).unwrap();
//! ```
//!
//! For templates loaded dynamically (JSON manifests, on-disk sources),
//! use [`Engine::register_template_with_schema`] to get the same check
//! at registration time.
//!
//! # Feature flags
//!
//! - `std` (default): `std::error::Error` on `ProsaicError`, `SystemTime::now()`
//!   fallbacks. Disable for `no_std + alloc` targets.
//! - `time` (default): `{ts|relative}` and `{ts|since_last}` pipes.
//! - `polish` (default): sentence-length budgeting and smart quotes.
//! - `reg` (default): referring expression generation (Dale-Reiter + graph-based).
//! - `serde` (off): `Serialize`/`Deserialize` on public types.
//! - `parallel` (off): `DocumentPlan::render_parallel` via rayon.
//!
//! # `no_std` support
//!
//! Disable the `std` feature to compile under `no_std + alloc`:
//!
//! ```toml
//! prosaic-core = { version = "0.2", default-features = false }
//! ```
//!
//! Without the `std` feature:
//! - `Variation::Random` falls back to `Variation::Fixed` (variant 0).
//! - `{ts|relative}` and `{ts|since_last}` require `engine.reference_time()`.
//! - `ProsaicError` does not implement `std::error::Error`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod agreement;
mod antonyms;
mod builder;
mod collections;
mod context;
mod discourse;
mod document;
mod engine;
mod error;
mod faithfulness;
mod hedge;
mod language;
#[cfg(feature = "polish")]
mod length;
mod proportion;
#[cfg(feature = "polish")]
mod punctuation;
mod quantify;
#[cfg(feature = "reg")]
mod reg;
pub mod rst;
mod salience;
mod session;
mod synonyms;
mod template;
#[cfg(feature = "time")]
mod time;

pub use agreement::{
    AgreementFeatures, AgreementPerson, Animacy, Case, Definiteness, Gender,
    Number as GrammaticalNumber,
};
pub use context::{Context, EntityValue, HasProsaicSchema, IntoValue, Value, entity};
pub use faithfulness::{FaithfulnessScore, PolarityDrift, score_faithfulness};
pub use language::{
    Aspect, Conjunction, Language, Mood, Person, PluralCategory, Tense, VerbForm, Voice,
    english_verb_phrase,
};
// assert_faithful! is exported via #[macro_export] in faithfulness.rs
pub use antonyms::{AntonymRegistry, insert_not};
pub use builder::{Clause, Sentence, Subject, named, subject};
pub use context::IntoContext;
pub use discourse::{Cf, DiscourseState, ListStyle, ReferenceForm, Transition};
pub use document::{
    DocumentPlan, GroupingStrategy, Paragraph, RhetoricalCategory, default_classifier,
};
#[cfg(feature = "reg")]
pub use engine::RegAlgorithm;
pub use engine::{Engine, RenderExplanation, RenderIter, Strictness, VariantScore, Variation};
pub use error::ProsaicError;
pub use hedge::{HedgeMode, hedge};
#[cfg(feature = "polish")]
pub use length::split_long;
pub use proportion::english_proportion;
pub use prosaic_common::{ValueType, pipe_spec, schema_lookup, types_compatible};
#[cfg(feature = "polish")]
pub use punctuation::{em_dash_nested_parentheticals, smart_quotes};
pub use quantify::{QuantifyMode, quantify};
#[cfg(feature = "reg")]
pub use reg::{
    EntityDescriptor, EntityRegistry, SubgraphDescription, distinguishing_attributes,
    distinguishing_subgraph,
};

// Implementation details consumed by the `prosaic_template!` macro at
// expansion time. Not intended for direct use — the public contract goes
// through `ValueType` and `HasProsaicSchema`.
#[doc(hidden)]
pub use prosaic_common::{PIPE_SPECS, PipeSpec};
pub use rst::RstRelation;
pub use salience::{Salience, SalienceThresholds};
pub use session::Session;
pub use synonyms::SynonymRegistry;
pub use template::{BareSegment, Pipe, PipeArg, Template};
#[cfg(feature = "time")]
pub use time::format_relative;

#[cfg(test)]
mod common_reexport_tests {
    //! Sanity checks that `prosaic-common`'s public surface is visible
    //! through `prosaic-core`'s re-exports. Each test invokes a re-exported
    //! fn through a non-trivial code path so a broken re-export or a
    //! behavioural regression in `prosaic-common` surfaces here.

    use super::*;

    #[test]
    fn pipe_specs_length_matches_registry() {
        assert_eq!(PIPE_SPECS.len(), 20);
    }

    #[test]
    fn pipe_spec_lookup_round_trips_through_reexport() {
        let p = pipe_spec("pluralize").expect("pluralize must resolve via re-export");
        assert_eq!(p.input, ValueType::Number);
        assert_eq!(p.output, ValueType::String);
    }

    #[test]
    fn types_compatible_via_reexport_rejects_mismatches() {
        // Any + concrete → compatible; distinct concretes → not.
        assert!(types_compatible(ValueType::Any, ValueType::Number));
        assert!(!types_compatible(ValueType::Number, ValueType::List));
    }

    #[test]
    fn schema_lookup_via_reexport_finds_keys() {
        let schema: &[(&str, ValueType)] = &[("x", ValueType::Number)];
        assert_eq!(schema_lookup(schema, "x"), Some(ValueType::Number));
        assert_eq!(schema_lookup(schema, "missing"), None);
    }

    #[test]
    fn pipe_spec_struct_is_constructible_via_reexport() {
        // Confirms the struct re-export is usable as a type, not just a name.
        let p = PipeSpec {
            name: "test",
            input: ValueType::Any,
            output: ValueType::String,
        };
        assert_eq!(p.name, "test");
        assert_eq!(p.input, ValueType::Any);
    }
}
