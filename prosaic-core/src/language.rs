#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::format;

/// Verb tense for conjugation.
///
/// Simple tense axis — combine with [`Aspect`] and [`Voice`] to get richer
/// forms like "has been renamed" (present perfect passive) or "is being
/// renamed" (present progressive passive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tense {
    #[default]
    Past,
    Present,
    Future,
}

/// Grammatical aspect — whether the action is simple, completed, or ongoing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Aspect {
    /// "renamed" / "was renamed" — a plain, point-in-time action.
    #[default]
    Simple,
    /// "has renamed" / "has been renamed" — emphasises completion/relevance.
    Perfect,
    /// "is renaming" / "is being renamed" — emphasises ongoing action.
    Progressive,
}

/// Voice controls whether the verb is rendered in active or passive form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Voice {
    /// "Foo renamed Foobar" — subject performs the action.
    Active,
    /// "Foo was renamed to Foobar" — subject receives the action. Default.
    #[default]
    Passive,
}

/// Grammatical person for conjugation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Person {
    First,
    Second,
    #[default]
    Third,
}

/// Grammatical mood — indicative (factual) vs conditional ("would …").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mood {
    #[default]
    Indicative,
    /// Conditional: "would rename" / "would be renamed". Only pairs with
    /// Simple or Perfect aspect; ignores tense.
    Conditional,
}

/// Conjunction used when joining lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Conjunction {
    And,
    Or,
}

/// A fully-specified verb form. Convenience enum covering the most common
/// tense × aspect × mood combinations in English. Use with
/// [`Language::verb_phrase`] or template `{…|verb:<form>}` pipes.
///
/// Each variant maps cleanly to a (Tense, Aspect, Mood) triple — see
/// [`VerbForm::resolve`]. Names are written from an English perspective
/// but the trait-level composition is language-agnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VerbForm {
    /// "renamed" / "was renamed"
    SimplePast,
    /// "renames" / "is renamed"
    SimplePresent,
    /// "will rename" / "will be renamed"
    SimpleFuture,
    /// "has renamed" / "has been renamed"
    PresentPerfect,
    /// "had renamed" / "had been renamed"
    PastPerfect,
    /// "will have renamed" / "will have been renamed"
    FuturePerfect,
    /// "is renaming" / "is being renamed"
    PresentProgressive,
    /// "was renaming" / "was being renamed"
    PastProgressive,
    /// "would rename" / "would be renamed"
    Conditional,
    /// "would have renamed" / "would have been renamed"
    ConditionalPerfect,
}

impl VerbForm {
    /// Decompose into tense, aspect, and mood primitives.
    pub fn resolve(self) -> (Tense, Aspect, Mood) {
        use Aspect::*;
        use Mood::*;
        use Tense::*;
        match self {
            VerbForm::SimplePast => (Past, Simple, Indicative),
            VerbForm::SimplePresent => (Present, Simple, Indicative),
            VerbForm::SimpleFuture => (Future, Simple, Indicative),
            VerbForm::PresentPerfect => (Present, Perfect, Indicative),
            VerbForm::PastPerfect => (Past, Perfect, Indicative),
            VerbForm::FuturePerfect => (Future, Perfect, Indicative),
            VerbForm::PresentProgressive => (Present, Progressive, Indicative),
            VerbForm::PastProgressive => (Past, Progressive, Indicative),
            // Conditional mood ignores tense; Present is used as a neutral slot.
            VerbForm::Conditional => (Present, Simple, Conditional),
            VerbForm::ConditionalPerfect => (Present, Perfect, Conditional),
        }
    }

    /// Parse a snake_case form name (e.g. "present_perfect") plus an
    /// optional `active_` prefix into `(VerbForm, Voice)`. Returns `None`
    /// on unknown names. Used by the `verb` template pipe.
    pub fn parse_spec(spec: &str) -> Option<(VerbForm, Voice)> {
        let (voice, rest) = if let Some(tail) = spec.strip_prefix("active_") {
            (Voice::Active, tail)
        } else if let Some(tail) = spec.strip_prefix("passive_") {
            (Voice::Passive, tail)
        } else {
            (Voice::Passive, spec)
        };

        let form = match rest {
            "past" | "simple_past" => VerbForm::SimplePast,
            "present" | "simple_present" => VerbForm::SimplePresent,
            "future" | "simple_future" => VerbForm::SimpleFuture,
            "present_perfect" => VerbForm::PresentPerfect,
            "past_perfect" => VerbForm::PastPerfect,
            "future_perfect" => VerbForm::FuturePerfect,
            "present_progressive" | "progressive" => VerbForm::PresentProgressive,
            "past_progressive" => VerbForm::PastProgressive,
            "conditional" => VerbForm::Conditional,
            "conditional_perfect" => VerbForm::ConditionalPerfect,
            _ => return None,
        };

        Some((form, voice))
    }
}

impl From<Tense> for VerbForm {
    fn from(tense: Tense) -> Self {
        match tense {
            Tense::Past => VerbForm::SimplePast,
            Tense::Present => VerbForm::SimplePresent,
            Tense::Future => VerbForm::SimpleFuture,
        }
    }
}

/// CLDR plural categories. A language's [`Language::plural_category`]
/// implementation maps an integer count into one of these six buckets.
/// The subset a language actually uses depends on its grammar:
///
/// - English: `One` | `Other`
/// - Spanish: `One` | `Other`
/// - Polish:  `One` | `Few` | `Many` | `Other`
/// - Arabic:  `Zero` | `One` | `Two` | `Few` | `Many` | `Other`
///
/// Non-English grammars must override [`Language::plural_category`] and
/// [`Language::pluralize_with_category`] to return the correct category and
/// the correct word form for their language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    /// The catch-all category; used by languages that do not distinguish a
    /// more specific bucket for a given count.
    #[default]
    Other,
}

/// Trait abstracting over a natural language's grammar rules.
///
/// Implement this trait for each language you want to support.
/// The crate ships with an English implementation in `prosaic-grammar-en`.
pub trait Language: Send + Sync {
    /// Return the plural form of `word` for the given `count`.
    /// When `count` is 1, return the singular form.
    fn pluralize(&self, word: &str, count: usize) -> String;

    /// Return the singular form of a potentially plural `word`.
    fn singularize(&self, word: &str) -> String;

    /// Return the indefinite article ("a" or "an" in English) for `word`.
    fn article(&self, word: &str) -> &str;

    /// Conjugate `verb` in the given simple `tense` and `person`
    /// (no aspect/mood compounding — just "renamed", "renames", "will rename").
    fn conjugate(&self, verb: &str, tense: Tense, person: Person) -> String;

    /// Return the past participle of a verb (e.g., "broken", "renamed").
    /// Used for passive and perfect constructions ("was renamed",
    /// "has been renamed", "will have broken").
    fn past_participle(&self, verb: &str) -> String;

    /// Return the present participle of a verb (e.g., "renaming", "writing").
    /// Used for progressive constructions ("is renaming", "was being renamed").
    fn present_participle(&self, verb: &str) -> String;

    /// Join a list of items with the given conjunction.
    /// Should use the language's standard list format (e.g., Oxford comma in English).
    fn join_list(&self, items: &[&str], conjunction: Conjunction) -> String;

    /// Return the ordinal string for `n` (e.g., "1st", "2nd", "3rd").
    fn ordinal(&self, n: usize) -> String;

    /// Spell out `n` as words (e.g., 42 → "forty-two").
    fn number_to_words(&self, n: usize) -> String;

    /// Produce a plural description for a set of same-type entities.
    ///
    /// Called by the `|refer` pipe when the slot value is a `Value::List` of
    /// 2+ items sharing an entity type. The default implementation is
    /// English-shaped but generic enough to serve as a reasonable fallback for
    /// languages that have not overridden it:
    ///
    /// - count = 0 → `""` (empty string)
    /// - count = 1 → `"the {entity_type}"`
    /// - count ≥ 2 → `"the {count} {entity_type_plural}"`
    ///
    /// Non-English grammars should override this to handle gender agreement
    /// (Spanish/French), counter words (Japanese), dual/few/many categories
    /// (Arabic), and so on. The `features` parameter carries agreement info
    /// propagated from the first entity in the set; override implementations
    /// may use it to select the correct article or adjective endings.
    ///
    /// English's `prosaic_grammar_en::English` uses the default; no override
    /// is needed for v1.
    fn plural_description(
        &self,
        entity_type: &str,
        count: usize,
        _features: &crate::agreement::AgreementFeatures,
    ) -> String {
        match count {
            0 => String::new(),
            1 => format!("the {entity_type}"),
            _ => format!("the {count} {}", self.pluralize(entity_type, count)),
        }
    }

    /// Render a full verb phrase combining tense, aspect, voice, and mood.
    ///
    /// Default implementation composes from the primitive inflections
    /// (`conjugate`, `past_participle`, `present_participle`) following
    /// English auxiliary-verb rules. Override for languages whose verb
    /// phrase structure differs from English's `aux + aux + participle`
    /// layout.
    fn verb_phrase(
        &self,
        verb: &str,
        form: VerbForm,
        voice: Voice,
        person: Person,
    ) -> String {
        english_verb_phrase(self, verb, form, voice, person)
    }

    /// Classify an integer count into a CLDR plural category.
    ///
    /// Default implementation uses English rules: `n == 1` → [`PluralCategory::One`],
    /// anything else → [`PluralCategory::Other`]. Non-English grammars must
    /// override this method to return the correct categories for their
    /// language (e.g., Polish distinguishes `One / Few / Many / Other`;
    /// Arabic uses all six categories).
    fn plural_category(&self, n: i64) -> PluralCategory {
        match n {
            1 => PluralCategory::One,
            _ => PluralCategory::Other,
        }
    }

    /// Produce the form of `word` appropriate for the given plural category.
    ///
    /// Default implementation uses English rules: [`PluralCategory::One`]
    /// returns the singular form (the word unchanged); any other category
    /// returns the plural form via [`Language::pluralize`] with count 2,
    /// which picks the plural branch in the legacy API without triggering
    /// irregular-specific overrides that count on the exact integer.
    ///
    /// Non-English grammars must override this method when they support
    /// richer category sets (e.g., Polish `One / Few / Many / Other`) or
    /// when gender / case agreement affects the choice of form.
    fn pluralize_with_category(&self, word: &str, category: PluralCategory) -> String {
        match category {
            PluralCategory::One => word.to_string(),
            _ => self.pluralize(word, 2),
        }
    }

    /// Return a discourse marker for the given RST relation.
    ///
    /// Emitted at the START of a sentence (with trailing space), e.g.
    /// `"Furthermore, "`. Return `None` to suppress the marker (the renderer
    /// will fall back to a plain inter-sentence space).
    ///
    /// The default implementation encodes English markers. Non-English grammars
    /// override with locale-appropriate markers.
    fn discourse_marker(&self, relation: crate::rst::RstRelation) -> Option<&'static str> {
        use crate::rst::RstRelation::*;
        Some(match relation {
            Elaboration => "Furthermore, ",
            Contrast    => "However, ",
            Cause       => "Because of this, ",
            Result      => "As a result, ",
            Concession  => "Nevertheless, ",
            Sequence    => "Then, ",
            Condition   => "If this happens, ",
            Background  => "Meanwhile, ",
            Summary     => "In summary, ",
        })
    }

    /// Format an inter-event temporal delta as a narrative phrase.
    ///
    /// Called by the `{…|since_last}` pipe when the session has a
    /// `last_temporal_anchor`. `diff_secs` is `current_ts - anchor_ts`
    /// (positive = later event). Zero or negative returns "at the same
    /// time" (English default); override for other languages.
    ///
    /// The default implementation produces English phrases like
    /// "the next day", "moments later", "3 weeks later". Non-English
    /// grammars should override to produce locale-appropriate phrases.
    #[cfg(feature = "time")]
    fn since_last_marker(&self, diff_secs: i64) -> String {
        crate::time::format_since_last(diff_secs)
    }

    /// Realize a reference form as surface text for this language.
    ///
    /// The discourse policy layer chooses the [`crate::discourse::ReferenceForm`]
    /// (Full, ShortName, Pronoun, Demonstrative, or Zero) based on
    /// language-agnostic rules. This method converts that choice into the
    /// language-specific surface string.
    ///
    /// Only `Pronoun`, `Demonstrative`, and `Zero` are meaningfully handled
    /// here. `Full` and `ShortName` route through the engine's REG layer
    /// (Dale & Reiter, graph-based) because they involve entity-attribute
    /// logic that's not the language's concern.
    ///
    /// Returns:
    /// - `Some(text)` for a realized form (e.g., `"it"`, `"they"`, `"this"`).
    /// - `None` for `Zero` (pro-drop) or for `Full`/`ShortName` — the caller
    ///   handles those via REG.
    ///
    /// The default implementation encodes English:
    /// - `Pronoun`: `"they"` when `features.number` is `Plural` or `Dual`,
    ///   `"it"` otherwise.
    /// - `Demonstrative`: `"this"`.
    /// - `Zero`: `None` (English doesn't drop pronouns).
    /// - `Full` / `ShortName`: `None` (engine handles via REG).
    fn realize_reference(
        &self,
        form: crate::discourse::ReferenceForm,
        features: &crate::agreement::AgreementFeatures,
    ) -> Option<String> {
        use crate::agreement::Number;
        use crate::discourse::ReferenceForm;
        match form {
            ReferenceForm::Pronoun => Some(match features.number {
                Number::Plural | Number::Dual => "they".to_string(),
                _ => "it".to_string(),
            }),
            ReferenceForm::Demonstrative => Some("this".to_string()),
            ReferenceForm::Zero => None,
            ReferenceForm::Full | ReferenceForm::ShortName => None,
        }
    }
}

/// Default English-style verb phrase composition. Provided as a free
/// function so custom `Language` impls can delegate to it selectively
/// (`fn verb_phrase(…) { english_verb_phrase(self, …) }`).
pub fn english_verb_phrase<L: Language + ?Sized>(
    lang: &L,
    verb: &str,
    form: VerbForm,
    voice: Voice,
    person: Person,
) -> String {
    let (tense, aspect, mood) = form.resolve();
    let past_participle = lang.past_participle(verb);
    let present_participle = lang.present_participle(verb);

    // "has" / "have" depends on person; use the language's own conjugation
    // so this still works with any `Language` that overrides `conjugate`.
    let have_aux = lang.conjugate("have", Tense::Present, person);
    let had_aux = "had";
    let be_present = lang.conjugate("be", Tense::Present, person);
    let be_past = match person {
        Person::Third | Person::First => "was".to_string(),
        Person::Second => "were".to_string(),
    };

    match mood {
        Mood::Conditional => match (aspect, voice) {
            (Aspect::Simple, Voice::Active) => format!("would {verb}"),
            (Aspect::Simple, Voice::Passive) => format!("would be {past_participle}"),
            (Aspect::Perfect, Voice::Active) => format!("would have {past_participle}"),
            (Aspect::Perfect, Voice::Passive) => {
                format!("would have been {past_participle}")
            }
            (Aspect::Progressive, Voice::Active) => format!("would be {present_participle}"),
            (Aspect::Progressive, Voice::Passive) => {
                format!("would be being {past_participle}")
            }
        },
        Mood::Indicative => match (tense, aspect, voice) {
            // Simple
            (Tense::Past, Aspect::Simple, Voice::Active) => {
                lang.conjugate(verb, Tense::Past, person)
            }
            (Tense::Past, Aspect::Simple, Voice::Passive) => {
                format!("{be_past} {past_participle}")
            }
            (Tense::Present, Aspect::Simple, Voice::Active) => {
                lang.conjugate(verb, Tense::Present, person)
            }
            (Tense::Present, Aspect::Simple, Voice::Passive) => {
                format!("{be_present} {past_participle}")
            }
            (Tense::Future, Aspect::Simple, Voice::Active) => format!("will {verb}"),
            (Tense::Future, Aspect::Simple, Voice::Passive) => {
                format!("will be {past_participle}")
            }

            // Perfect
            (Tense::Past, Aspect::Perfect, Voice::Active) => {
                format!("{had_aux} {past_participle}")
            }
            (Tense::Past, Aspect::Perfect, Voice::Passive) => {
                format!("{had_aux} been {past_participle}")
            }
            (Tense::Present, Aspect::Perfect, Voice::Active) => {
                format!("{have_aux} {past_participle}")
            }
            (Tense::Present, Aspect::Perfect, Voice::Passive) => {
                format!("{have_aux} been {past_participle}")
            }
            (Tense::Future, Aspect::Perfect, Voice::Active) => {
                format!("will have {past_participle}")
            }
            (Tense::Future, Aspect::Perfect, Voice::Passive) => {
                format!("will have been {past_participle}")
            }

            // Progressive
            (Tense::Past, Aspect::Progressive, Voice::Active) => {
                format!("{be_past} {present_participle}")
            }
            (Tense::Past, Aspect::Progressive, Voice::Passive) => {
                format!("{be_past} being {past_participle}")
            }
            (Tense::Present, Aspect::Progressive, Voice::Active) => {
                format!("{be_present} {present_participle}")
            }
            (Tense::Present, Aspect::Progressive, Voice::Passive) => {
                format!("{be_present} being {past_participle}")
            }
            (Tense::Future, Aspect::Progressive, Voice::Active) => {
                format!("will be {present_participle}")
            }
            (Tense::Future, Aspect::Progressive, Voice::Passive) => {
                // "will be being renamed" is technically valid but awkward;
                // callers rarely want it. Composed anyway for completeness.
                format!("will be being {past_participle}")
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agreement::AgreementFeatures;
    use super::PluralCategory;

    /// Minimal Language implementation used only in unit tests for this module.
    struct MiniLang;

    impl Language for MiniLang {
        fn pluralize(&self, word: &str, count: usize) -> String {
            if count == 1 {
                return word.to_string();
            }
            // Basic English pluralisation rules for common test words.
            if word.ends_with("ss") || word.ends_with("sh") || word.ends_with("ch") || word.ends_with('x') || word.ends_with('z') {
                format!("{word}es")
            } else if word.ends_with('s') {
                // e.g. "class" → "classes"
                format!("{word}es")
            } else {
                format!("{word}s")
            }
        }
        fn singularize(&self, word: &str) -> String {
            word.strip_suffix('s').unwrap_or(word).to_string()
        }
        fn article(&self, _word: &str) -> &str {
            "a"
        }
        fn conjugate(&self, verb: &str, tense: Tense, _person: Person) -> String {
            match tense {
                Tense::Past => format!("{verb}ed"),
                Tense::Present => verb.to_string(),
                Tense::Future => format!("will {verb}"),
            }
        }
        fn past_participle(&self, verb: &str) -> String {
            format!("{verb}ed")
        }
        fn present_participle(&self, verb: &str) -> String {
            format!("{verb}ing")
        }
        fn join_list(&self, items: &[&str], _conjunction: Conjunction) -> String {
            items.join(", ")
        }
        fn ordinal(&self, n: usize) -> String {
            format!("{n}th")
        }
        fn number_to_words(&self, n: usize) -> String {
            format!("{n}")
        }
    }

    #[test]
    fn plural_description_default_zero_is_empty() {
        let l = MiniLang;
        assert_eq!(
            l.plural_description("class", 0, &AgreementFeatures::default()),
            ""
        );
    }

    #[test]
    fn plural_description_default_one_is_the_type() {
        let l = MiniLang;
        assert_eq!(
            l.plural_description("class", 1, &AgreementFeatures::default()),
            "the class"
        );
    }

    #[test]
    fn plural_description_default_many_uses_pluralize() {
        let l = MiniLang;
        assert_eq!(
            l.plural_description("class", 3, &AgreementFeatures::default()),
            "the 3 classes"
        );
    }

    #[test]
    fn plural_description_two_items() {
        let l = MiniLang;
        assert_eq!(
            l.plural_description("service", 2, &AgreementFeatures::default()),
            "the 2 services"
        );
    }

    #[test]
    fn plural_description_ignores_features_in_default_impl() {
        // The default impl does not use features — result must be identical
        // regardless of what features are passed. Languages that care will override.
        let l = MiniLang;
        let with_features = l.plural_description("class", 3, &AgreementFeatures::default());
        let without = l.plural_description("class", 3, &AgreementFeatures::default());
        assert_eq!(with_features, without);
        assert_eq!(with_features, "the 3 classes");
    }

    // ── PluralCategory + default trait methods ───────────────────────────────

    #[test]
    fn default_plural_category_one() {
        let lang = MiniLang;
        assert_eq!(lang.plural_category(1), PluralCategory::One);
    }

    #[test]
    fn default_plural_category_other_for_zero() {
        let lang = MiniLang;
        assert_eq!(lang.plural_category(0), PluralCategory::Other);
    }

    #[test]
    fn default_plural_category_other_for_plurals() {
        let lang = MiniLang;
        assert_eq!(lang.plural_category(2), PluralCategory::Other);
        assert_eq!(lang.plural_category(17), PluralCategory::Other);
    }

    #[test]
    fn default_plural_category_other_for_negatives() {
        let lang = MiniLang;
        assert_eq!(lang.plural_category(-5), PluralCategory::Other);
    }

    #[test]
    fn default_pluralize_with_category_one_is_singular() {
        let lang = MiniLang;
        assert_eq!(
            lang.pluralize_with_category("service", PluralCategory::One),
            "service"
        );
    }

    #[test]
    fn default_pluralize_with_category_other_is_plural() {
        let lang = MiniLang;
        assert_eq!(
            lang.pluralize_with_category("service", PluralCategory::Other),
            "services"
        );
    }

    #[test]
    fn default_pluralize_with_category_few_falls_to_plural() {
        // English collapses Few/Many/Zero/Two to Other; the default impl
        // routes all non-One categories to the plural form.
        let lang = MiniLang;
        assert_eq!(
            lang.pluralize_with_category("service", PluralCategory::Few),
            "services"
        );
    }

    #[test]
    fn default_pluralize_with_category_many_falls_to_plural() {
        let lang = MiniLang;
        assert_eq!(
            lang.pluralize_with_category("service", PluralCategory::Many),
            "services"
        );
    }

    #[test]
    fn default_pluralize_with_category_zero_falls_to_plural() {
        let lang = MiniLang;
        assert_eq!(
            lang.pluralize_with_category("service", PluralCategory::Zero),
            "services"
        );
    }

    #[test]
    fn default_pluralize_with_category_two_falls_to_plural() {
        let lang = MiniLang;
        assert_eq!(
            lang.pluralize_with_category("service", PluralCategory::Two),
            "services"
        );
    }

    #[test]
    fn plural_category_default_variant_is_other() {
        assert_eq!(PluralCategory::default(), PluralCategory::Other);
    }

    // ── realize_reference default implementation ─────────────────────────────

    #[test]
    fn realize_reference_pronoun_singular() {
        let lang = MiniLang;
        let f = AgreementFeatures::default(); // number=Unknown → falls through to "it"
        assert_eq!(
            lang.realize_reference(crate::discourse::ReferenceForm::Pronoun, &f),
            Some("it".to_string())
        );
    }

    #[test]
    fn realize_reference_pronoun_plural() {
        let lang = MiniLang;
        let f = AgreementFeatures::default().with_number(crate::agreement::Number::Plural);
        assert_eq!(
            lang.realize_reference(crate::discourse::ReferenceForm::Pronoun, &f),
            Some("they".to_string())
        );
    }

    #[test]
    fn realize_reference_pronoun_dual() {
        let lang = MiniLang;
        let f = AgreementFeatures::default().with_number(crate::agreement::Number::Dual);
        assert_eq!(
            lang.realize_reference(crate::discourse::ReferenceForm::Pronoun, &f),
            Some("they".to_string())
        );
    }

    #[test]
    fn realize_reference_demonstrative() {
        let lang = MiniLang;
        let f = AgreementFeatures::default();
        assert_eq!(
            lang.realize_reference(crate::discourse::ReferenceForm::Demonstrative, &f),
            Some("this".to_string())
        );
    }

    #[test]
    fn realize_reference_zero_is_none() {
        let lang = MiniLang;
        let f = AgreementFeatures::default();
        assert_eq!(
            lang.realize_reference(crate::discourse::ReferenceForm::Zero, &f),
            None
        );
    }

    #[test]
    fn realize_reference_full_is_none() {
        // Full form is handled by engine REG, not the language layer.
        let lang = MiniLang;
        let f = AgreementFeatures::default();
        assert_eq!(
            lang.realize_reference(crate::discourse::ReferenceForm::Full, &f),
            None
        );
    }

    #[test]
    fn realize_reference_short_name_is_none() {
        // ShortName is handled by engine REG, not the language layer.
        let lang = MiniLang;
        let f = AgreementFeatures::default();
        assert_eq!(
            lang.realize_reference(crate::discourse::ReferenceForm::ShortName, &f),
            None
        );
    }

    #[test]
    fn discourse_marker_english_defaults() {
        let lang = MiniLang;
        use crate::rst::RstRelation::*;
        assert_eq!(lang.discourse_marker(Elaboration), Some("Furthermore, "));
        assert_eq!(lang.discourse_marker(Contrast),    Some("However, "));
        assert_eq!(lang.discourse_marker(Result),      Some("As a result, "));
    }

    // ── since_last_marker default (English) ──────────────────────────────────

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_default_the_next_day() {
        let lang = MiniLang;
        assert_eq!(lang.since_last_marker(86_400 + 1), "the next day");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_default_moments_later() {
        let lang = MiniLang;
        assert_eq!(lang.since_last_marker(30), "moments later");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_default_zero_is_at_the_same_time() {
        let lang = MiniLang;
        assert_eq!(lang.since_last_marker(0), "at the same time");
    }

    #[cfg(feature = "time")]
    #[test]
    fn since_last_marker_default_years() {
        let lang = MiniLang;
        assert_eq!(lang.since_last_marker(3 * 365 * 86_400), "3 years later");
    }
}
