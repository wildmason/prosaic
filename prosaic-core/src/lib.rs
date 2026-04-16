//! General-purpose natural language generation from structured data.
//!
//! Takes structured events and produces **natural-sounding** English
//! text, not just grammatically correct output. The engine tracks
//! discourse state across calls, so multiple renders flow together like
//! human-written prose — using pronouns, varying phrasing, matching
//! verbosity to impact, and structuring multi-paragraph narratives.
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

mod language;
mod context;
pub mod agreement;
mod error;
mod template;
mod engine;
mod faithfulness;
mod builder;
mod discourse;
mod session;
mod salience;
mod document;
#[cfg(feature = "reg")]
mod reg;
mod synonyms;
#[cfg(feature = "time")]
mod time;
mod quantify;
mod hedge;
mod antonyms;
#[cfg(feature = "polish")]
mod length;
#[cfg(feature = "polish")]
mod punctuation;

pub use language::{
    english_verb_phrase, Aspect, Conjunction, Language, Mood, Person, Tense, VerbForm, Voice,
};
pub use agreement::{
    AgreementFeatures, AgreementPerson, Animacy, Case, Definiteness, Gender, Number as GrammaticalNumber,
};
pub use context::{Context, IntoValue, Value};
pub use faithfulness::{score_faithfulness, FaithfulnessScore, PolarityDrift};
// assert_faithful! is exported via #[macro_export] in faithfulness.rs
pub use error::ProsaicError;
pub use template::{Template, Pipe, PipeArg};
pub use engine::{Engine, RenderExplanation, RenderIter, Strictness, VariantScore, Variation};
#[cfg(feature = "reg")]
pub use engine::RegAlgorithm;
pub use session::Session;
pub use builder::{Sentence, Clause, Subject, entity, named};
pub use context::IntoContext;
pub use discourse::{ListStyle, ReferenceForm};
pub use salience::{Salience, SalienceThresholds};
pub use document::{
    default_classifier, DocumentPlan, GroupingStrategy, Paragraph, RhetoricalCategory,
};
#[cfg(feature = "reg")]
pub use reg::{
    distinguishing_attributes, distinguishing_subgraph,
    EntityDescriptor, EntityRegistry, SubgraphDescription,
};
pub use synonyms::SynonymRegistry;
#[cfg(feature = "time")]
pub use time::format_relative;
pub use quantify::{quantify, QuantifyMode};
pub use hedge::{hedge, HedgeMode};
pub use antonyms::{insert_not, AntonymRegistry};
#[cfg(feature = "polish")]
pub use length::split_long;
#[cfg(feature = "polish")]
pub use punctuation::{em_dash_nested_parentheticals, smart_quotes};
