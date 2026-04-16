//! Hedging / confidence language.
//!
//! Turn a confidence score into a hedge word that reflects certainty.
//! Input is an integer 0..=100 representing a percentage. For callers
//! working with floats in the 0.0–1.0 range, multiply by 100 and round
//! before passing in.

/// Which form the hedge should take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HedgeMode {
    /// Adverb: "certainly", "likely", "probably", "possibly", "perhaps".
    /// Fits templates like "X {conf|hedge} broke the build".
    #[default]
    Adverb,
    /// Modal verb: "must", "should", "may", "might", "could". Fits
    /// templates like "X {conf|hedge:modal} break the build".
    Modal,
    /// Prefix phrase: "It is certain that", "It is likely that",
    /// "Probably", "Possibly", "Perhaps". Fits templates where the
    /// hedge leads a clause.
    Prefix,
}

/// Map a 0..=100 score into a hedge word for the given mode.
/// Values outside the range are clamped.
pub fn hedge(score: i64, mode: HedgeMode) -> &'static str {
    let bucket = bucket_of(score);
    match (mode, bucket) {
        (HedgeMode::Adverb, Bucket::Certain) => "certainly",
        (HedgeMode::Adverb, Bucket::Likely) => "likely",
        (HedgeMode::Adverb, Bucket::Probable) => "probably",
        (HedgeMode::Adverb, Bucket::Possible) => "possibly",
        (HedgeMode::Adverb, Bucket::Unlikely) => "perhaps",

        (HedgeMode::Modal, Bucket::Certain) => "must",
        (HedgeMode::Modal, Bucket::Likely) => "should",
        (HedgeMode::Modal, Bucket::Probable) => "may",
        (HedgeMode::Modal, Bucket::Possible) => "might",
        (HedgeMode::Modal, Bucket::Unlikely) => "could",

        (HedgeMode::Prefix, Bucket::Certain) => "it is certain that",
        (HedgeMode::Prefix, Bucket::Likely) => "it is likely that",
        (HedgeMode::Prefix, Bucket::Probable) => "probably",
        (HedgeMode::Prefix, Bucket::Possible) => "possibly",
        (HedgeMode::Prefix, Bucket::Unlikely) => "perhaps",
    }
}

/// Parse a mode spec ("adverb", "modal", "prefix") from a pipe argument.
pub fn parse_mode(spec: &str) -> Option<HedgeMode> {
    match spec {
        "adverb" => Some(HedgeMode::Adverb),
        "modal" => Some(HedgeMode::Modal),
        "prefix" => Some(HedgeMode::Prefix),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
enum Bucket {
    Certain,
    Likely,
    Probable,
    Possible,
    Unlikely,
}

fn bucket_of(score: i64) -> Bucket {
    match score.clamp(0, 100) {
        90..=100 => Bucket::Certain,
        70..=89 => Bucket::Likely,
        50..=69 => Bucket::Probable,
        30..=49 => Bucket::Possible,
        _ => Bucket::Unlikely,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adverb_buckets() {
        assert_eq!(hedge(95, HedgeMode::Adverb), "certainly");
        assert_eq!(hedge(80, HedgeMode::Adverb), "likely");
        assert_eq!(hedge(60, HedgeMode::Adverb), "probably");
        assert_eq!(hedge(40, HedgeMode::Adverb), "possibly");
        assert_eq!(hedge(10, HedgeMode::Adverb), "perhaps");
    }

    #[test]
    fn modal_buckets() {
        assert_eq!(hedge(95, HedgeMode::Modal), "must");
        assert_eq!(hedge(80, HedgeMode::Modal), "should");
        assert_eq!(hedge(60, HedgeMode::Modal), "may");
        assert_eq!(hedge(40, HedgeMode::Modal), "might");
        assert_eq!(hedge(10, HedgeMode::Modal), "could");
    }

    #[test]
    fn prefix_buckets() {
        assert_eq!(hedge(95, HedgeMode::Prefix), "it is certain that");
        assert_eq!(hedge(80, HedgeMode::Prefix), "it is likely that");
        assert_eq!(hedge(60, HedgeMode::Prefix), "probably");
        assert_eq!(hedge(40, HedgeMode::Prefix), "possibly");
        assert_eq!(hedge(10, HedgeMode::Prefix), "perhaps");
    }

    #[test]
    fn out_of_range_scores_clamp() {
        assert_eq!(hedge(-50, HedgeMode::Adverb), "perhaps");
        assert_eq!(hedge(200, HedgeMode::Adverb), "certainly");
    }

    #[test]
    fn bucket_boundaries() {
        assert_eq!(hedge(89, HedgeMode::Adverb), "likely");
        assert_eq!(hedge(90, HedgeMode::Adverb), "certainly");
        assert_eq!(hedge(69, HedgeMode::Adverb), "probably");
        assert_eq!(hedge(70, HedgeMode::Adverb), "likely");
    }
}
