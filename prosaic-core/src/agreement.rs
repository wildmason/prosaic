//! Grammatical agreement features for multilingual rendering.
//!
//! Carries gender / number / case / definiteness / animacy / person
//! metadata on entity-typed context values. The English grammar layer
//! ignores these features entirely; non-English grammars (`-es`, `-de`,
//! etc.) consult them to produce correctly-agreeing articles, adjectives,
//! pronouns, and verb forms.

/// Grammatical gender axis.
///
/// Defaults to [`Gender::Unknown`] so English callers pay no cognitive cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Gender {
    #[default]
    Unknown,
    Masc,
    Fem,
    Neut,
    /// Dutch, Scandinavian 2-gender ("common" + "neuter") systems.
    Common,
}

/// Grammatical number axis.
///
/// Defaults to [`Number::Unknown`] so English callers pay no cognitive cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Number {
    #[default]
    Unknown,
    Singular,
    Plural,
    /// Arabic, Slovenian, Biblical Hebrew dual number.
    Dual,
}

/// Grammatical case axis.
///
/// Defaults to [`Case::Unknown`]. Additional cases (instrumental, locative,
/// ablative, etc.) are reserved for v2 when Finnish/Russian/etc. become targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Case {
    #[default]
    Unknown,
    Nominative,
    Accusative,
    Dative,
    Genitive,
}

/// Definiteness axis.
///
/// Defaults to [`Definiteness::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Definiteness {
    #[default]
    Unknown,
    Definite,
    Indefinite,
}

/// Animacy axis (relevant to Russian/Polish case declension and similar).
///
/// Defaults to [`Animacy::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Animacy {
    #[default]
    Unknown,
    Animate,
    Inanimate,
}

/// Grammatical person axis for agreement feature metadata.
///
/// Defaults to [`AgreementPerson::Third`], which is correct for most
/// named entities (services, components, etc.).
///
/// Note: this is distinct from [`crate::language::Person`], which drives
/// verb conjugation in the English grammar layer. This enum carries
/// agreement metadata on [`crate::Value::Entity`] values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AgreementPerson {
    First,
    Second,
    #[default]
    Third,
}

/// A bundle of grammatical agreement features for a named entity.
///
/// All fields default to their "unknown" / most-neutral variant so that English
/// callers can ignore the struct entirely while non-English grammars consult it
/// for gender, number, case, definiteness, animacy, and person.
///
/// # Example
///
/// ```
/// use prosaic_core::agreement::{AgreementFeatures, Gender, Number, Definiteness};
///
/// let f = AgreementFeatures::new()
///     .with_gender(Gender::Fem)
///     .with_number(Number::Singular)
///     .with_definiteness(Definiteness::Definite);
///
/// assert_eq!(f.gender, Gender::Fem);
/// assert_eq!(f.number, Number::Singular);
/// assert_eq!(f.definiteness, Definiteness::Definite);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AgreementFeatures {
    pub gender: Gender,
    pub number: Number,
    pub case: Case,
    pub definiteness: Definiteness,
    pub animacy: Animacy,
    pub person: AgreementPerson,
}

impl AgreementFeatures {
    /// Create a new `AgreementFeatures` with all fields set to their defaults
    /// (all Unknown / Third person). Equivalent to `Default::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the gender axis.
    pub fn with_gender(mut self, g: Gender) -> Self {
        self.gender = g;
        self
    }

    /// Set the number axis.
    pub fn with_number(mut self, n: Number) -> Self {
        self.number = n;
        self
    }

    /// Set the case axis.
    pub fn with_case(mut self, c: Case) -> Self {
        self.case = c;
        self
    }

    /// Set the definiteness axis.
    pub fn with_definiteness(mut self, d: Definiteness) -> Self {
        self.definiteness = d;
        self
    }

    /// Set the animacy axis.
    pub fn with_animacy(mut self, a: Animacy) -> Self {
        self.animacy = a;
        self
    }

    /// Set the person axis.
    pub fn with_person(mut self, p: AgreementPerson) -> Self {
        self.person = p;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_unknown() {
        let f = AgreementFeatures::default();
        assert_eq!(f.gender, Gender::Unknown);
        assert_eq!(f.number, Number::Unknown);
        assert_eq!(f.case, Case::Unknown);
        assert_eq!(f.definiteness, Definiteness::Unknown);
        assert_eq!(f.animacy, Animacy::Unknown);
        assert_eq!(f.person, AgreementPerson::Third);
    }

    #[test]
    fn builder_with_methods_set_fields() {
        let f = AgreementFeatures::new()
            .with_gender(Gender::Fem)
            .with_number(Number::Singular)
            .with_case(Case::Accusative)
            .with_definiteness(Definiteness::Definite)
            .with_animacy(Animacy::Animate)
            .with_person(AgreementPerson::First);
        assert_eq!(f.gender, Gender::Fem);
        assert_eq!(f.number, Number::Singular);
        assert_eq!(f.case, Case::Accusative);
        assert_eq!(f.definiteness, Definiteness::Definite);
        assert_eq!(f.animacy, Animacy::Animate);
        assert_eq!(f.person, AgreementPerson::First);
    }

    #[test]
    fn features_are_copy() {
        fn takes_copy<T: Copy>(_: T) {}
        takes_copy(AgreementFeatures::default());
        takes_copy(Gender::Fem);
        takes_copy(Number::Plural);
        takes_copy(Case::Dative);
        takes_copy(Definiteness::Indefinite);
        takes_copy(Animacy::Animate);
        takes_copy(AgreementPerson::First);
    }

    #[test]
    fn all_gender_variants_are_distinct() {
        assert_ne!(Gender::Masc, Gender::Fem);
        assert_ne!(Gender::Fem, Gender::Neut);
        assert_ne!(Gender::Neut, Gender::Common);
        assert_ne!(Gender::Common, Gender::Unknown);
    }

    #[test]
    fn all_number_variants_are_distinct() {
        assert_ne!(Number::Singular, Number::Plural);
        assert_ne!(Number::Plural, Number::Dual);
        assert_ne!(Number::Dual, Number::Unknown);
    }

    #[test]
    fn all_case_variants_are_distinct() {
        assert_ne!(Case::Nominative, Case::Accusative);
        assert_ne!(Case::Accusative, Case::Dative);
        assert_ne!(Case::Dative, Case::Genitive);
        assert_ne!(Case::Genitive, Case::Unknown);
    }
}
