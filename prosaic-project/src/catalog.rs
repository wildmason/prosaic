//! Reference catalog of [`StyleProfile`]s.
//!
//! Four ready-made profiles ship with `prosaic-project` to give new
//! projects a starting point and to provide stable test fixtures. They
//! are designed to be authored examples — copy-paste-and-edit material —
//! rather than runtime defaults. Projects that want one applied verbatim
//! can do so via [`StyleProfile::clone`] on the catalog accessor's
//! return value, or by referencing them from a `prosaic.toml` profile
//! file via `extends = "..."`.
//!
//! The four profiles are deliberately well-separated along their dial
//! axes so the rendered prose for the same input is visibly different
//! between any two. The `style_profile_catalog` integration test
//! enforces that separation.

use prosaic_core::{
    ConnectivePreferences, HedgingCalibration, LengthDistribution, ListStyleBias, PronounDensity,
    RstRelation, SalienceBias, StyleProfile, Verbosity,
};

/// The byte-for-byte-equivalent baseline. Exists in the catalog so
/// consumers can reference `Catalog::neutral()` symmetrically with the
/// other presets and so test fixtures have a "no profile" entry to
/// compare against.
pub fn neutral() -> StyleProfile {
    StyleProfile::neutral()
}

/// Concise, professional register — short and direct, formal pronoun
/// usage, slightly more confident hedging. Designed for engineering
/// status posts, terse change notes, executive summaries.
pub fn concise_professional() -> StyleProfile {
    let mut connectives = ConnectivePreferences::neutral();
    connectives.allowed.insert(
        RstRelation::Elaboration,
        vec!["Furthermore,".to_string(), "Additionally,".to_string()],
    );
    connectives
        .allowed
        .insert(RstRelation::Contrast, vec!["However,".to_string()]);

    StyleProfile::builder("concise-professional")
        .verbosity(Verbosity::Terse)
        .list_style_bias(ListStyleBias::Bracketed)
        .pronoun_density(PronounDensity::Low)
        .salience(SalienceBias::Auto)
        .hedging(HedgingCalibration {
            offset: 5,
            forbid: Vec::new(),
        })
        .sentence_length(LengthDistribution {
            short: 0.5,
            medium: 0.4,
            long: 0.1,
            short_max_words: 8,
            medium_max_words: 18,
        })
        .connectives(connectives)
        .build()
        .expect("concise-professional must validate")
}

/// Verbose, narrative register — longer flowing sentences, looser
/// pronoun usage, including-style list openers, slightly less confident
/// hedging. Designed for tutorials, end-user documentation, conversational
/// release notes.
pub fn verbose_narrative() -> StyleProfile {
    StyleProfile::builder("verbose-narrative")
        .verbosity(Verbosity::Verbose)
        .list_style_bias(ListStyleBias::Including)
        .pronoun_density(PronounDensity::High)
        .salience(SalienceBias::Auto)
        .hedging(HedgingCalibration {
            offset: -5,
            forbid: Vec::new(),
        })
        .sentence_length(LengthDistribution {
            short: 0.2,
            medium: 0.5,
            long: 0.3,
            short_max_words: 8,
            medium_max_words: 18,
        })
        .build()
        .expect("verbose-narrative must validate")
}

/// Regulatory-formal register — long sentences, no pronouns, careful
/// hedging that bans absolutes, bracketed enumeration for precision.
/// Designed for compliance reports, audit summaries, anything where
/// precision and conservativism dominate over readability.
pub fn regulatory_formal() -> StyleProfile {
    StyleProfile::builder("regulatory-formal")
        .verbosity(Verbosity::Verbose)
        .list_style_bias(ListStyleBias::Bracketed)
        .pronoun_density(PronounDensity::Low)
        .salience(SalienceBias::Auto)
        .hedging(HedgingCalibration {
            offset: -10,
            forbid: vec!["certainly".to_string(), "must".to_string()],
        })
        .sentence_length(LengthDistribution {
            short: 0.1,
            medium: 0.5,
            long: 0.4,
            short_max_words: 8,
            medium_max_words: 18,
        })
        .build()
        .expect("regulatory-formal must validate")
}

/// All catalog profiles, in the canonical ordering used by tests and
/// documentation: neutral first, then the three voiced presets.
pub fn all() -> Vec<StyleProfile> {
    vec![
        neutral(),
        concise_professional(),
        verbose_narrative(),
        regulatory_formal(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_catalog_entry_is_neutral() {
        assert!(neutral().is_neutral());
    }

    #[test]
    fn concise_professional_validates() {
        let p = concise_professional();
        p.validate().unwrap();
        assert_eq!(p.name, "concise-professional");
        assert_eq!(p.verbosity, Verbosity::Terse);
        assert!(!p.is_neutral());
    }

    #[test]
    fn verbose_narrative_validates() {
        let p = verbose_narrative();
        p.validate().unwrap();
        assert_eq!(p.name, "verbose-narrative");
        assert_eq!(p.verbosity, Verbosity::Verbose);
        assert!(!p.is_neutral());
    }

    #[test]
    fn regulatory_formal_validates() {
        let p = regulatory_formal();
        p.validate().unwrap();
        assert_eq!(p.name, "regulatory-formal");
        assert!(!p.hedging.forbid.is_empty());
        assert!(!p.is_neutral());
    }

    #[test]
    fn all_returns_four_profiles_with_distinct_names() {
        let profiles = all();
        assert_eq!(profiles.len(), 4);
        let mut names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "concise-professional",
                "neutral",
                "regulatory-formal",
                "verbose-narrative",
            ]
        );
    }
}
