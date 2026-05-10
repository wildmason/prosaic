//! `StyleProfile` — the deterministic dial layer that biases the engine's
//! existing rendering choices toward a target voice.
//!
//! A profile is a struct of small, orthogonal dials. Each dial maps to one
//! or two specific decision sites the engine already makes; setting a dial
//! shifts the bias of that decision rather than replacing it. The profile
//! is immutable for the lifetime of an [`Engine`](crate::Engine) and is
//! consulted at decision time — there is no learned state, no per-render
//! mutation, and no profile-conditional template content.
//!
//! See `docs/superpowers/specs/2026-05-09-style-profile-design.md` for the
//! full design rationale and the resolved decisions on each dial.

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::collections::{HashMap, new_map};
use crate::rst::RstRelation;

/// Verbosity dial — biases salience-tier preference at variant selection.
///
/// `Terse` prefers the lowest-detail variant available among the candidates
/// already filtered for the rendering context's salience. `Verbose` prefers
/// the highest. `Neutral` defers to the engine's existing salience-from-
/// context logic with no bias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Verbosity {
    Terse,
    #[default]
    Neutral,
    Verbose,
}

/// Sentence-length distribution target.
///
/// Used as a soft prior — the rhythm scorer treats the profile's distribution
/// as a nudge alongside its existing repetition + cadence terms; the
/// Self-Refine retro-pass uses it as one signal among several rather than
/// a hard constraint. Values are interpreted as proportions and need not
/// sum to 1.0; the scorer normalizes internally. Boundary thresholds in
/// words are configurable so a profile can declare what "short" means for
/// its target voice.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LengthDistribution {
    /// Target proportion of sentences with word count `<= short_max_words`.
    pub short: f32,
    /// Target proportion of sentences with word count `<= medium_max_words`
    /// but `> short_max_words`.
    pub medium: f32,
    /// Target proportion of sentences with word count `> medium_max_words`.
    pub long: f32,
    /// Inclusive upper bound, in words, that classifies a sentence as short.
    pub short_max_words: u16,
    /// Inclusive upper bound, in words, that classifies a sentence as
    /// medium. Must be `>= short_max_words` (validated at construction).
    pub medium_max_words: u16,
}

impl LengthDistribution {
    /// Neutral distribution — uniform target with no shape preference.
    /// Equivalent to "no profile target," and what `StyleProfile::neutral()`
    /// returns. Boundary defaults (`short_max_words = 8`,
    /// `medium_max_words = 18`) match the rhythm scorer's working ranges.
    pub fn neutral() -> Self {
        Self {
            short: 1.0 / 3.0,
            medium: 1.0 / 3.0,
            long: 1.0 / 3.0,
            short_max_words: 8,
            medium_max_words: 18,
        }
    }

    /// Returns `true` when this distribution is the neutral default —
    /// the rhythm scorer's profile-aware path short-circuits when it is,
    /// preserving byte-for-byte equivalence with the no-profile path.
    pub fn is_neutral(&self) -> bool {
        // Compare on raw bits to avoid f32 epsilon drift across rebuilds.
        let neutral = Self::neutral();
        self.short.to_bits() == neutral.short.to_bits()
            && self.medium.to_bits() == neutral.medium.to_bits()
            && self.long.to_bits() == neutral.long.to_bits()
            && self.short_max_words == neutral.short_max_words
            && self.medium_max_words == neutral.medium_max_words
    }
}

impl Default for LengthDistribution {
    fn default() -> Self {
        Self::neutral()
    }
}

/// Per-RST-relation connective preferences.
///
/// `allowed` restricts which connectives the engine may pick for a given
/// RST relation. A missing key means the engine's default pool for that
/// relation is used unmodified. An explicit empty `Vec` for a key is
/// rejected at validation time — empty pools are a footgun that would
/// silently force fallback every time.
///
/// `preferred` is a tie-breaker layer applied within whatever candidate
/// set survives `allowed`-filtering: connectives that match the
/// `preferred` list for a relation get their weights summed into the
/// scorer; connectives without an entry score 0 (uniform). Weights are
/// additive and do not normalize.
///
/// Family-budget enforcement (the existing trailing-window cap on emissions
/// per connector family) runs unchanged. The profile narrows the candidate
/// set; the budget governs rotation within whatever set survives.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConnectivePreferences {
    pub allowed: HashMap<RstRelation, Vec<String>>,
    pub preferred: HashMap<RstRelation, Vec<(String, f32)>>,
}

impl ConnectivePreferences {
    /// An empty preferences struct — every relation falls through to the
    /// engine's default pool and uniform weights.
    pub fn neutral() -> Self {
        Self {
            allowed: new_map(),
            preferred: new_map(),
        }
    }

    /// `true` when no relation has an explicit entry in either map.
    pub fn is_neutral(&self) -> bool {
        self.allowed.is_empty() && self.preferred.is_empty()
    }
}

/// List-style cycle tiebreaker.
///
/// When the engine's `{items|join}` rotation has multiple candidates that
/// haven't been used recently, this dial breaks the tie. `Auto` (default)
/// preserves the existing rotation. The other variants nudge toward a
/// specific opener while still respecting the anti-repeat rule — you cannot
/// use a profile to force one style every time; that's what
/// `{items|join:bracketed}` is for. The profile sets the prior, not the
/// verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ListStyleBias {
    #[default]
    Auto,
    Including,
    SuchAs,
    Dash,
    Bracketed,
}

/// Pronoun density dial — adjusts the threshold at which `{name|refer}`
/// switches from full form to short form to pronoun.
///
/// `Low` keeps full forms longer (formal register). `High` switches to
/// pronouns earlier (conversational register). Implementation is a small
/// offset on the existing centering-theory transition rules; the rules
/// themselves don't change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PronounDensity {
    Low,
    #[default]
    Default,
    High,
}

/// Hedging calibration — shifts the deterministic confidence-to-hedge
/// mapping.
///
/// `offset` is added to the input confidence (clamped to `0..=100`) before
/// the bucket lookup. `forbid` removes specific hedges from the available
/// set; per the resolved decision in the design spec, `forbid` falls
/// through *toward more confident* — forbidding a hedge usually expresses
/// "I don't want wishy-washy framing here," and the firmer-fallback
/// matches that intent.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HedgingCalibration {
    /// `-50..=+50` added to confidence before hedge mapping.
    pub offset: i8,
    /// Hedge words/phrases to never emit (case-insensitive). Match is
    /// against the raw hedge string the engine would otherwise return
    /// (`"perhaps"`, `"likely"`, `"it is certain that"`, etc.).
    pub forbid: Vec<String>,
}

impl HedgingCalibration {
    pub fn neutral() -> Self {
        Self {
            offset: 0,
            forbid: Vec::new(),
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.offset == 0 && self.forbid.is_empty()
    }
}

/// Salience-threshold bias.
///
/// Composes with [`Verbosity`]; the documented composition order is
/// `SalienceBias` runs first (changing tier classification), then
/// `Verbosity` runs (preferring within-tier or cross-tier per its
/// setting). `Lower` shifts the engine's salience thresholds *down* so
/// the same numeric inputs land in *higher* tiers; `Higher` shifts them
/// *up* so inputs land in *lower* tiers. Both produce qualitatively
/// different prose without touching the underlying classification logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SalienceBias {
    /// Lower thresholds → contexts land in higher tiers more often.
    Lower,
    #[default]
    Auto,
    /// Higher thresholds → contexts land in lower tiers more often.
    Higher,
}

/// Validation error returned by [`StyleProfile::validate`] and
/// `StyleProfileBuilder::build`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StyleProfileError {
    /// `connectives.allowed[relation] = []` was set explicitly. An empty
    /// pool would force fallback every time — almost always a footgun.
    EmptyAllowedPool { relation: RstRelation },
    /// `length_distribution.medium_max_words < length_distribution.short_max_words`.
    InvalidLengthBoundaries {
        short_max_words: u16,
        medium_max_words: u16,
    },
    /// `hedging.offset` outside the documented `-50..=+50` range.
    HedgingOffsetOutOfRange { offset: i8 },
    /// Length-distribution proportion was negative or non-finite.
    InvalidLengthProportion { which: &'static str, value: f32 },
}

impl core::fmt::Display for StyleProfileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StyleProfileError::EmptyAllowedPool { relation } => write!(
                f,
                "style profile: connectives.allowed[{relation:?}] is an explicit empty pool"
            ),
            StyleProfileError::InvalidLengthBoundaries {
                short_max_words,
                medium_max_words,
            } => write!(
                f,
                "style profile: medium_max_words ({medium_max_words}) must be >= short_max_words ({short_max_words})"
            ),
            StyleProfileError::HedgingOffsetOutOfRange { offset } => write!(
                f,
                "style profile: hedging.offset {offset} outside documented range -50..=+50"
            ),
            StyleProfileError::InvalidLengthProportion { which, value } => write!(
                f,
                "style profile: length_distribution.{which} = {value} is negative or non-finite"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StyleProfileError {}

/// A declarative voice configuration for the engine.
///
/// Profiles are immutable per engine. Build one through
/// [`StyleProfile::builder`], a serde-deserialized `prosaic.toml`
/// `[style_profile]` section, or one of the catalog presets. Apply via
/// [`Engine::style_profile`](crate::Engine::style_profile).
///
/// `StyleProfile::neutral()` is the byte-for-byte-equivalent baseline:
/// constructing an engine with `.style_profile(StyleProfile::neutral())`
/// produces output identical to constructing it with no profile at all.
/// This invariant is the backwards-compatibility gate for the entire
/// design and is asserted by the round-trip tests at the workspace level.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct StyleProfile {
    pub name: String,
    pub verbosity: Verbosity,
    pub sentence_length: LengthDistribution,
    pub connectives: ConnectivePreferences,
    pub list_style_bias: ListStyleBias,
    pub pronoun_density: PronounDensity,
    pub hedging: HedgingCalibration,
    pub salience: SalienceBias,
}

impl StyleProfile {
    /// The byte-for-byte-equivalent baseline. Every dial in its neutral
    /// position; constructing an engine with this profile produces the
    /// same output as constructing it with no profile at all.
    pub fn neutral() -> Self {
        Self {
            name: String::from("neutral"),
            verbosity: Verbosity::default(),
            sentence_length: LengthDistribution::neutral(),
            connectives: ConnectivePreferences::neutral(),
            list_style_bias: ListStyleBias::default(),
            pronoun_density: PronounDensity::default(),
            hedging: HedgingCalibration::neutral(),
            salience: SalienceBias::default(),
        }
    }

    /// Start a builder rooted at `neutral()` with the given name.
    pub fn builder(name: impl Into<String>) -> StyleProfileBuilder {
        StyleProfileBuilder {
            profile: Self {
                name: name.into(),
                ..Self::neutral()
            },
        }
    }

    /// `true` when this profile is byte-for-byte equivalent to the neutral
    /// baseline — i.e., when every decision site can short-circuit its
    /// profile-aware path. The `name` field is ignored: a custom name
    /// applied to neutral dials is still an effective no-op.
    pub fn is_neutral(&self) -> bool {
        self.verbosity == Verbosity::default()
            && self.sentence_length.is_neutral()
            && self.connectives.is_neutral()
            && self.list_style_bias == ListStyleBias::default()
            && self.pronoun_density == PronounDensity::default()
            && self.hedging.is_neutral()
            && self.salience == SalienceBias::default()
    }

    /// Validate the profile against the documented invariants. Called by
    /// the builder's `build()` and at the engine boundary; consumers
    /// passing a deserialized profile should call this before applying
    /// it to an engine.
    pub fn validate(&self) -> Result<(), StyleProfileError> {
        for (relation, pool) in &self.connectives.allowed {
            if pool.is_empty() {
                return Err(StyleProfileError::EmptyAllowedPool {
                    relation: *relation,
                });
            }
        }
        if self.sentence_length.medium_max_words < self.sentence_length.short_max_words {
            return Err(StyleProfileError::InvalidLengthBoundaries {
                short_max_words: self.sentence_length.short_max_words,
                medium_max_words: self.sentence_length.medium_max_words,
            });
        }
        for (which, value) in [
            ("short", self.sentence_length.short),
            ("medium", self.sentence_length.medium),
            ("long", self.sentence_length.long),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(StyleProfileError::InvalidLengthProportion { which, value });
            }
        }
        if !(-50..=50).contains(&self.hedging.offset) {
            return Err(StyleProfileError::HedgingOffsetOutOfRange {
                offset: self.hedging.offset,
            });
        }
        Ok(())
    }
}

impl Default for StyleProfile {
    fn default() -> Self {
        Self::neutral()
    }
}

/// Builder for [`StyleProfile`].
///
/// Construct via [`StyleProfile::builder`] and chain dial setters; finalize
/// with `build()`, which runs [`StyleProfile::validate`] before returning
/// the profile so misconfigurations surface at construction rather than at
/// render time.
#[derive(Debug, Clone)]
pub struct StyleProfileBuilder {
    profile: StyleProfile,
}

impl StyleProfileBuilder {
    pub fn verbosity(mut self, v: Verbosity) -> Self {
        self.profile.verbosity = v;
        self
    }

    pub fn sentence_length(mut self, distribution: LengthDistribution) -> Self {
        self.profile.sentence_length = distribution;
        self
    }

    pub fn connectives(mut self, prefs: ConnectivePreferences) -> Self {
        self.profile.connectives = prefs;
        self
    }

    /// Append an `allowed`-pool entry for one RST relation.
    pub fn allow_connectives(
        mut self,
        relation: RstRelation,
        pool: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let pool: Vec<String> = pool.into_iter().map(Into::into).collect();
        self.profile.connectives.allowed.insert(relation, pool);
        self
    }

    /// Append a `preferred`-weight entry for one RST relation.
    pub fn prefer_connectives(
        mut self,
        relation: RstRelation,
        weights: impl IntoIterator<Item = (impl Into<String>, f32)>,
    ) -> Self {
        let weights: Vec<(String, f32)> =
            weights.into_iter().map(|(s, w)| (s.into(), w)).collect();
        self.profile.connectives.preferred.insert(relation, weights);
        self
    }

    pub fn list_style_bias(mut self, bias: ListStyleBias) -> Self {
        self.profile.list_style_bias = bias;
        self
    }

    pub fn pronoun_density(mut self, density: PronounDensity) -> Self {
        self.profile.pronoun_density = density;
        self
    }

    pub fn hedging(mut self, calibration: HedgingCalibration) -> Self {
        self.profile.hedging = calibration;
        self
    }

    pub fn forbid_hedge(mut self, hedge: impl Into<String>) -> Self {
        self.profile.hedging.forbid.push(hedge.into());
        self
    }

    pub fn hedging_offset(mut self, offset: i8) -> Self {
        self.profile.hedging.offset = offset;
        self
    }

    pub fn salience(mut self, bias: SalienceBias) -> Self {
        self.profile.salience = bias;
        self
    }

    /// Validate and finalize. Returns [`StyleProfileError`] if any dial
    /// violates the documented invariants.
    pub fn build(self) -> Result<StyleProfile, StyleProfileError> {
        self.profile.validate()?;
        Ok(self.profile)
    }

    /// Validate-or-panic finalize; useful in const-style top-level catalogs
    /// where a panic is preferable to a `?` chain. Production builders
    /// should prefer [`Self::build`].
    pub fn build_or_panic(self) -> StyleProfile {
        self.profile.validate().expect("style profile validation");
        self.profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_round_trips_through_default() {
        let n = StyleProfile::neutral();
        let d = StyleProfile::default();
        assert_eq!(n, d);
        assert!(n.is_neutral());
    }

    #[test]
    fn builder_named_profile_with_no_dial_changes_is_neutral_in_effect() {
        // Spec invariant: a profile whose dials are all at their neutral
        // values is byte-for-byte equivalent to no profile, regardless of
        // name. `is_neutral()` ignores name.
        let p = StyleProfile::builder("custom").build().unwrap();
        assert_eq!(p.name, "custom");
        assert!(p.is_neutral());
    }

    #[test]
    fn builder_with_changed_dial_is_not_neutral() {
        let p = StyleProfile::builder("terse")
            .verbosity(Verbosity::Terse)
            .build()
            .unwrap();
        assert!(!p.is_neutral());
    }

    #[test]
    fn empty_allowed_pool_is_rejected() {
        let p = StyleProfile::builder("bad")
            .allow_connectives(RstRelation::Contrast, Vec::<String>::new())
            .build();
        assert!(matches!(
            p,
            Err(StyleProfileError::EmptyAllowedPool {
                relation: RstRelation::Contrast
            })
        ));
    }

    #[test]
    fn allow_connectives_with_entries_validates() {
        let p = StyleProfile::builder("ok")
            .allow_connectives(RstRelation::Contrast, ["However", "Conversely"])
            .build()
            .unwrap();
        assert_eq!(
            p.connectives
                .allowed
                .get(&RstRelation::Contrast)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn invalid_length_boundaries_rejected() {
        let bad = LengthDistribution {
            short_max_words: 20,
            medium_max_words: 10,
            ..LengthDistribution::neutral()
        };
        let result = StyleProfile::builder("bad").sentence_length(bad).build();
        assert!(matches!(
            result,
            Err(StyleProfileError::InvalidLengthBoundaries { .. })
        ));
    }

    #[test]
    fn invalid_length_proportion_rejected() {
        let bad = LengthDistribution {
            short: -0.1,
            ..LengthDistribution::neutral()
        };
        let result = StyleProfile::builder("bad").sentence_length(bad).build();
        assert!(matches!(
            result,
            Err(StyleProfileError::InvalidLengthProportion { which: "short", .. })
        ));
    }

    #[test]
    fn hedging_offset_out_of_range_rejected() {
        let result = StyleProfile::builder("bad").hedging_offset(75).build();
        assert!(matches!(
            result,
            Err(StyleProfileError::HedgingOffsetOutOfRange { offset: 75 })
        ));
    }

    #[test]
    fn neutral_validates() {
        StyleProfile::neutral().validate().expect("neutral must validate");
    }

    #[test]
    fn dial_changes_independent() {
        // Each dial moves out of neutral on its own, leaving the others
        // alone — guards against accidentally coupled defaults.
        let v = StyleProfile::builder("v")
            .verbosity(Verbosity::Verbose)
            .build()
            .unwrap();
        assert_eq!(v.verbosity, Verbosity::Verbose);
        assert!(v.connectives.is_neutral());
        assert!(v.hedging.is_neutral());
        assert_eq!(v.salience, SalienceBias::Auto);
    }
}
