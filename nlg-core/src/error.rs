use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
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

    /// Returned when the faithfulness gate is enabled and the rendered output
    /// fails the precision threshold or has a polarity mismatch. The session
    /// state is rolled back as if the render had not occurred.
    #[error("faithfulness rejected: precision {precision:.3}, polarity_match {polarity_match}")]
    FaithfulnessRejection {
        precision: f32,
        polarity_match: bool,
    },
}
