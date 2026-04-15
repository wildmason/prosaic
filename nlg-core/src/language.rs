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

/// Trait abstracting over a natural language's grammar rules.
///
/// Implement this trait for each language you want to support.
/// The crate ships with an English implementation in `nlg-grammar-en`.
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
