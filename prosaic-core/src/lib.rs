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
#[cfg(feature = "polish")]
mod punctuation;
mod proportion;
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
pub use context::{Context, EntityValue, IntoValue, Value, entity};
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
#[cfg(feature = "polish")]
pub use punctuation::{em_dash_nested_parentheticals, smart_quotes};
pub use proportion::english_proportion;
pub use quantify::{QuantifyMode, quantify};
#[cfg(feature = "reg")]
pub use reg::{
    EntityDescriptor, EntityRegistry, SubgraphDescription, distinguishing_attributes,
    distinguishing_subgraph,
};
pub use rst::RstRelation;
pub use salience::{Salience, SalienceThresholds};
pub use session::Session;
pub use synonyms::SynonymRegistry;
pub use template::{BareSegment, Pipe, PipeArg, Template};
#[cfg(feature = "time")]
pub use time::format_relative;
