use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NlgError {
    #[error("missing slot `{slot}` in template `{template}`")]
    MissingSlot { template: String, slot: String },

    #[error("unknown template `{0}`")]
    UnknownTemplate(String),

    #[error("invalid pipe `{pipe}`: {reason}")]
    InvalidPipe { pipe: String, reason: String },

    #[error("grammar error: {0}")]
    GrammarError(String),

    #[error("template parse error in `{template}` at position {position}: {reason}")]
    TemplateParseError {
        template: String,
        position: usize,
        reason: String,
    },
}
