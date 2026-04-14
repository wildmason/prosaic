mod language;
mod context;
mod error;
mod template;
mod engine;
mod builder;

pub use language::{Language, Tense, Person, Conjunction};
pub use context::{Context, Value};
pub use error::NlgError;
pub use template::{Template, Pipe, PipeArg};
pub use engine::{Engine, Variation, Strictness};
pub use builder::{Sentence, Clause, Subject, entity, named};
pub use context::IntoContext;
