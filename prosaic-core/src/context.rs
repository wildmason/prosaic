use ahash::AHashMap;

use crate::agreement::AgreementFeatures;

/// A value that can be inserted into a rendering context.
///
/// The [`Value::Entity`] variant carries a named entity with optional
/// grammatical agreement features for multilingual rendering. In English
/// it renders identically to [`Value::String`] (just the name); non-English
/// grammars can inspect the `features` field for gender, number, case, etc.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
    String(String),
    Number(i64),
    List(Vec<String>),
    /// A named entity carrying agreement features for multilingual rendering.
    ///
    /// In English rendering this behaves identically to `Value::String(name)`.
    /// Non-English grammars consult `features` to produce correctly-agreeing
    /// articles, adjectives, pronouns, and verb forms.
    Entity {
        name: String,
        #[cfg_attr(feature = "serde", serde(default))]
        features: AgreementFeatures,
    },
}

impl Value {
    /// Render this value as a display string.
    ///
    /// For [`Value::Entity`] this returns the entity's name — identical
    /// to the behaviour of [`Value::String`].
    pub fn as_display(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::List(items) => items.join(", "),
            Value::Entity { name, .. } => name.clone(),
        }
    }

    /// Try to interpret this value as a number.
    pub fn as_number(&self) -> Option<i64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Try to interpret this value as a list of strings.
    ///
    /// Returns `None` for [`Value::Entity`] — entities are not lists.
    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }
}

/// Holds the key-value pairs passed to a template for rendering.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Context {
    values: AHashMap<String, Value>,
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a value into the context.
    pub fn insert(&mut self, key: impl Into<String>, value: Value) {
        self.values.insert(key.into(), value);
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    /// Iterate over all keys in the context.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    /// Iterate over all key-value pairs in the context.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// Convert a host value into a [`Value`]. Used by the `ctx!` macro to
/// accept strings, integers, and slices without requiring the caller to
/// wrap each in a `Value::*` constructor.
///
/// This trait is intentionally narrow: impls are shipped for the common host
/// types and third-party extension is not expected. For bespoke types, convert
/// to a supported primitive first (e.g. via `Display`) or use the longer form
/// `Value::String("...".into())`.
///
/// Note: `u64` is intentionally excluded — the conversion to `i64` can
/// overflow silently. Callers with a `u64` should cast explicitly (`v as i64`).
pub trait IntoValue {
    fn into_value(self) -> Value;
}

impl IntoValue for &str {
    fn into_value(self) -> Value {
        Value::String(self.to_string())
    }
}

impl IntoValue for String {
    fn into_value(self) -> Value {
        Value::String(self)
    }
}

impl IntoValue for &String {
    fn into_value(self) -> Value {
        Value::String(self.clone())
    }
}

macro_rules! impl_into_value_int {
    ($($t:ty),*) => {
        $(impl IntoValue for $t {
            fn into_value(self) -> Value { Value::Number(self as i64) }
        })*
    };
}
impl_into_value_int!(i8, i16, i32, i64, isize, u8, u16, u32, usize);

impl IntoValue for bool {
    fn into_value(self) -> Value {
        Value::Number(if self { 1 } else { 0 })
    }
}

impl IntoValue for Value {
    fn into_value(self) -> Value {
        self
    }
}

impl IntoValue for Vec<String> {
    fn into_value(self) -> Value {
        Value::List(self)
    }
}

impl IntoValue for Vec<&str> {
    fn into_value(self) -> Value {
        Value::List(self.iter().map(|s| s.to_string()).collect())
    }
}

impl<const N: usize> IntoValue for [&str; N] {
    fn into_value(self) -> Value {
        Value::List(self.iter().map(|s| s.to_string()).collect())
    }
}

impl<const N: usize> IntoValue for [String; N] {
    fn into_value(self) -> Value {
        Value::List(self.to_vec())
    }
}

/// Fluent builder for entity-typed context values with agreement features.
///
/// Produced by [`entity()`]; consumed into a [`Value::Entity`] via
/// [`IntoValue::into_value`] or [`EntityValue::build`].
///
/// # Example
///
/// ```
/// use prosaic_core::{ctx, entity, Value};
/// use prosaic_core::agreement::{Gender, Number, Definiteness};
///
/// let c = ctx! {
///     user: entity("Alice").fem().sing().defined(),
///     service: entity("UserService"),
/// };
///
/// match c.get("user").unwrap() {
///     Value::Entity { name, features } => {
///         assert_eq!(name, "Alice");
///         assert_eq!(features.gender, Gender::Fem);
///     }
///     _ => panic!("expected Entity"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityValue {
    name: String,
    features: crate::agreement::AgreementFeatures,
}

impl EntityValue {
    // ── Gender shortcuts ──────────────────────────────────────────────────

    /// Set gender to masculine.
    pub fn masc(mut self) -> Self {
        self.features.gender = crate::agreement::Gender::Masc;
        self
    }

    /// Set gender to feminine.
    pub fn fem(mut self) -> Self {
        self.features.gender = crate::agreement::Gender::Fem;
        self
    }

    /// Set gender to neuter.
    pub fn neut(mut self) -> Self {
        self.features.gender = crate::agreement::Gender::Neut;
        self
    }

    /// Set gender to common (Dutch / Scandinavian 2-gender systems).
    pub fn common(mut self) -> Self {
        self.features.gender = crate::agreement::Gender::Common;
        self
    }

    // ── Number shortcuts ──────────────────────────────────────────────────

    /// Set number to singular.
    pub fn sing(mut self) -> Self {
        self.features.number = crate::agreement::Number::Singular;
        self
    }

    /// Set number to plural.
    pub fn plur(mut self) -> Self {
        self.features.number = crate::agreement::Number::Plural;
        self
    }

    /// Set number to dual (Arabic, Slovenian, Biblical Hebrew).
    pub fn dual(mut self) -> Self {
        self.features.number = crate::agreement::Number::Dual;
        self
    }

    // ── Definiteness shortcuts ────────────────────────────────────────────

    /// Set definiteness to definite.
    pub fn defined(mut self) -> Self {
        self.features.definiteness = crate::agreement::Definiteness::Definite;
        self
    }

    /// Set definiteness to indefinite.
    pub fn indef(mut self) -> Self {
        self.features.definiteness = crate::agreement::Definiteness::Indefinite;
        self
    }

    // ── Animacy shortcuts ─────────────────────────────────────────────────

    /// Set animacy to animate.
    pub fn animate(mut self) -> Self {
        self.features.animacy = crate::agreement::Animacy::Animate;
        self
    }

    /// Set animacy to inanimate.
    pub fn inanimate(mut self) -> Self {
        self.features.animacy = crate::agreement::Animacy::Inanimate;
        self
    }

    // ── Case and person ───────────────────────────────────────────────────

    /// Set the grammatical case.
    pub fn case(mut self, c: crate::agreement::Case) -> Self {
        self.features.case = c;
        self
    }

    /// Set the grammatical person.
    pub fn person(mut self, p: crate::agreement::AgreementPerson) -> Self {
        self.features.person = p;
        self
    }

    // ── Full-feature override ─────────────────────────────────────────────

    /// Replace all features at once with a pre-built [`AgreementFeatures`].
    pub fn with_features(mut self, f: crate::agreement::AgreementFeatures) -> Self {
        self.features = f;
        self
    }

    // ── Consume ───────────────────────────────────────────────────────────

    /// Consume this builder into a [`Value::Entity`].
    pub fn build(self) -> Value {
        Value::Entity {
            name: self.name,
            features: self.features,
        }
    }
}

impl IntoValue for EntityValue {
    fn into_value(self) -> Value {
        self.build()
    }
}

/// Create an [`EntityValue`] for a named entity with default (unknown)
/// agreement features. Chain builder methods to set gender, number, etc.
///
/// ```
/// use prosaic_core::{ctx, entity, Value};
/// use prosaic_core::agreement::Gender;
///
/// let c = ctx! {
///     user: entity("Alice").fem().sing().defined(),
///     service: entity("UserService"),  // features stay Unknown — English default
/// };
/// match c.get("user").unwrap() {
///     Value::Entity { name, .. } => assert_eq!(name, "Alice"),
///     _ => panic!(),
/// }
/// ```
pub fn entity(name: impl Into<String>) -> EntityValue {
    EntityValue {
        name: name.into(),
        features: crate::agreement::AgreementFeatures::default(),
    }
}

/// Build a [`Context`] from key/value pairs. Values may be any type that
/// implements [`IntoValue`] — `&str`, `String`, integer types, `bool`,
/// `Vec<&str>`, `[&str; N]`, or an explicit `Value::*`.
///
/// Trailing commas are allowed. Empty `ctx! {}` produces an empty context.
///
/// # Example
///
/// ```
/// use prosaic_core::{ctx, Context, Value};
///
/// let c: Context = ctx! {
///     entity_type: "class",
///     name: "UserService",
///     consumer_count: 3,
///     consumers: ["ProfileComponent", "SettingsComponent", "AdminModule"],
/// };
///
/// assert_eq!(c.get("entity_type"), Some(&Value::String("class".into())));
/// assert_eq!(c.get("consumer_count"), Some(&Value::Number(3)));
/// ```
#[macro_export]
macro_rules! ctx {
    () => { $crate::Context::new() };
    ( $( $key:ident : $value:expr ),* $(,)? ) => {{
        let mut c = $crate::Context::new();
        $(
            $crate::Context::insert(&mut c, stringify!($key), $crate::IntoValue::into_value($value));
        )*
        c
    }};
}

/// Convenience trait for converting types into `Context`.
pub trait IntoContext {
    fn into_context(self) -> Context;
}

impl IntoContext for Context {
    fn into_context(self) -> Context {
        self
    }
}

// Allow passing &Context to render methods that accept impl IntoContext
// by cloning. This is ergonomic for callers that reuse a context.
impl IntoContext for &Context {
    fn into_context(self) -> Context {
        self.clone()
    }
}

#[cfg(test)]
mod into_value_tests {
    use super::*;

    #[test]
    fn str_becomes_value_string() {
        let v: Value = "hello".into_value();
        assert_eq!(v, Value::String("hello".into()));
    }

    #[test]
    fn owned_string_becomes_value_string_without_clone() {
        let s = String::from("hello");
        let v: Value = s.into_value();
        assert_eq!(v, Value::String("hello".into()));
    }

    #[test]
    fn borrowed_string_becomes_value_string() {
        let s = String::from("hello");
        let v: Value = (&s).into_value();
        assert_eq!(v, Value::String("hello".into()));
    }

    #[test]
    fn i64_becomes_value_number() {
        let v: Value = 42_i64.into_value();
        assert_eq!(v, Value::Number(42));
    }

    #[test]
    fn i32_becomes_value_number() {
        let v: Value = 42_i32.into_value();
        assert_eq!(v, Value::Number(42));
    }

    #[test]
    fn usize_becomes_value_number() {
        let v: Value = 7_usize.into_value();
        assert_eq!(v, Value::Number(7));
    }

    #[test]
    fn bool_becomes_number_zero_or_one() {
        assert_eq!(true.into_value(), Value::Number(1));
        assert_eq!(false.into_value(), Value::Number(0));
    }

    #[test]
    fn vec_of_str_becomes_value_list() {
        let v: Value = vec!["a", "b"].into_value();
        assert_eq!(v, Value::List(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn array_of_str_becomes_value_list() {
        let v: Value = ["a", "b"].into_value();
        assert_eq!(v, Value::List(vec!["a".into(), "b".into()]));
    }

    #[test]
    fn value_passes_through_identity() {
        let v = Value::Number(99);
        assert_eq!(v.clone().into_value(), v);
    }
}

#[cfg(test)]
mod ctx_macro_tests {
    use crate::{Context, Value};

    #[test]
    fn empty_ctx_is_empty() {
        let c: Context = ctx! {};
        assert_eq!(c.get("anything"), None);
    }

    #[test]
    fn single_slot() {
        let c = ctx! { name: "Foo" };
        assert_eq!(c.get("name"), Some(&Value::String("Foo".into())));
    }

    #[test]
    fn multiple_slots_mixed_types() {
        let c = ctx! {
            name: "Foo",
            count: 3,
            flag: true,
        };
        assert_eq!(c.get("name"), Some(&Value::String("Foo".into())));
        assert_eq!(c.get("count"), Some(&Value::Number(3)));
        assert_eq!(c.get("flag"), Some(&Value::Number(1)));
    }

    #[test]
    fn list_slot_from_array() {
        let c = ctx! { items: ["a", "b", "c"] };
        assert_eq!(
            c.get("items"),
            Some(&Value::List(vec!["a".into(), "b".into(), "c".into()]))
        );
    }

    #[test]
    fn trailing_comma_allowed() {
        let c = ctx! { a: 1, b: 2, };
        assert_eq!(c.get("a"), Some(&Value::Number(1)));
        assert_eq!(c.get("b"), Some(&Value::Number(2)));
    }

    #[test]
    fn expression_values_are_evaluated() {
        let s = String::from("dynamic");
        let c = ctx! { name: s };
        assert_eq!(c.get("name"), Some(&Value::String("dynamic".into())));
    }

    #[test]
    fn value_literal_passes_through() {
        let c = ctx! { x: Value::Number(7) };
        assert_eq!(c.get("x"), Some(&Value::Number(7)));
    }
}

#[cfg(test)]
mod entity_value_tests {
    use super::*;
    use crate::agreement::{AgreementFeatures, Gender, Number};

    #[test]
    fn entity_display_is_name() {
        let v = Value::Entity {
            name: "UserService".into(),
            features: AgreementFeatures::default(),
        };
        assert_eq!(v.as_display(), "UserService");
    }

    #[test]
    fn entity_as_list_is_none() {
        let v = Value::Entity {
            name: "X".into(),
            features: AgreementFeatures::default(),
        };
        assert!(v.as_list().is_none());
    }

    #[test]
    fn entity_as_number_is_none() {
        let v = Value::Entity {
            name: "Service".into(),
            features: AgreementFeatures::default(),
        };
        assert!(v.as_number().is_none());
    }

    #[test]
    fn entity_with_features_round_trips_via_equality() {
        let features = AgreementFeatures::new()
            .with_gender(Gender::Fem)
            .with_number(Number::Singular);
        let v1 = Value::Entity {
            name: "Alice".into(),
            features,
        };
        let v2 = v1.clone();
        assert_eq!(v1, v2);
    }

    #[test]
    fn entity_display_ignores_features() {
        // The name is all that as_display returns — features are invisible.
        let v_plain = Value::Entity {
            name: "Alice".into(),
            features: AgreementFeatures::default(),
        };
        let v_with_features = Value::Entity {
            name: "Alice".into(),
            features: AgreementFeatures::new()
                .with_gender(Gender::Fem)
                .with_number(Number::Singular),
        };
        assert_eq!(v_plain.as_display(), v_with_features.as_display());
    }
}

#[cfg(test)]
mod entity_builder_tests {
    use super::*;
    use crate::agreement::{AgreementFeatures, Animacy, Case, Definiteness, Gender, Number};

    #[test]
    fn entity_helper_default_features() {
        let ev = entity("UserService");
        let v = ev.into_value();
        match v {
            Value::Entity { name, features } => {
                assert_eq!(name, "UserService");
                assert_eq!(features, AgreementFeatures::default());
            }
            _ => panic!("expected Value::Entity"),
        }
    }

    #[test]
    fn entity_builder_chain_sets_features() {
        let v = entity("Alice")
            .fem()
            .sing()
            .defined()
            .animate()
            .into_value();
        match v {
            Value::Entity { name, features } => {
                assert_eq!(name, "Alice");
                assert_eq!(features.gender, Gender::Fem);
                assert_eq!(features.number, Number::Singular);
                assert_eq!(features.definiteness, Definiteness::Definite);
                assert_eq!(features.animacy, Animacy::Animate);
            }
            _ => panic!("expected Value::Entity"),
        }
    }

    #[test]
    fn entity_builder_all_gender_shortcuts() {
        assert_eq!(entity("x").masc().into_value(), Value::Entity {
            name: "x".into(),
            features: AgreementFeatures::new().with_gender(Gender::Masc),
        });
        assert_eq!(entity("x").fem().into_value(), Value::Entity {
            name: "x".into(),
            features: AgreementFeatures::new().with_gender(Gender::Fem),
        });
        assert_eq!(entity("x").neut().into_value(), Value::Entity {
            name: "x".into(),
            features: AgreementFeatures::new().with_gender(Gender::Neut),
        });
        assert_eq!(entity("x").common().into_value(), Value::Entity {
            name: "x".into(),
            features: AgreementFeatures::new().with_gender(Gender::Common),
        });
    }

    #[test]
    fn entity_builder_all_number_shortcuts() {
        assert_eq!(
            entity("x").plur().features.number,
            Number::Plural
        );
        assert_eq!(
            entity("x").dual().features.number,
            Number::Dual
        );
        assert_eq!(
            entity("x").sing().features.number,
            Number::Singular
        );
    }

    #[test]
    fn entity_builder_definiteness_shortcuts() {
        assert_eq!(
            entity("x").defined().features.definiteness,
            Definiteness::Definite
        );
        assert_eq!(
            entity("x").indef().features.definiteness,
            Definiteness::Indefinite
        );
    }

    #[test]
    fn entity_builder_animacy_shortcuts() {
        assert_eq!(
            entity("x").animate().features.animacy,
            Animacy::Animate
        );
        assert_eq!(
            entity("x").inanimate().features.animacy,
            Animacy::Inanimate
        );
    }

    #[test]
    fn entity_builder_case_method() {
        assert_eq!(
            entity("x").case(Case::Genitive).features.case,
            Case::Genitive
        );
    }

    #[test]
    fn entity_builder_with_features_override() {
        let full = AgreementFeatures::new()
            .with_gender(Gender::Fem)
            .with_number(Number::Plural);
        let v = entity("items").with_features(full).into_value();
        match v {
            Value::Entity { features, .. } => {
                assert_eq!(features.gender, Gender::Fem);
                assert_eq!(features.number, Number::Plural);
            }
            _ => panic!("expected Value::Entity"),
        }
    }

    #[test]
    fn ctx_macro_accepts_entity_value() {
        let c = ctx! {
            user: entity("Alice").fem().sing(),
            count: 3,
        };
        match c.get("user").unwrap() {
            Value::Entity { name, features } => {
                assert_eq!(name, "Alice");
                assert_eq!(features.gender, Gender::Fem);
            }
            _ => panic!("expected Value::Entity"),
        }
        assert_eq!(c.get("count"), Some(&Value::Number(3)));
    }

    #[test]
    fn entity_build_and_into_value_are_equivalent() {
        let ev1 = entity("TestService").fem();
        let ev2 = ev1.clone();
        assert_eq!(ev1.build(), ev2.into_value());
    }
}

