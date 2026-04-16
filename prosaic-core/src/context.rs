use ahash::AHashMap;

/// A value that can be inserted into a rendering context.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
    String(String),
    Number(i64),
    List(Vec<String>),
}

impl Value {
    /// Render this value as a display string.
    pub fn as_display(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::List(items) => items.join(", "),
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
    use crate::{ctx, Context, Value};

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
