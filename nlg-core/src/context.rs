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
}

/// Convert a host value into a [`Value`]. Used by the [`ctx!`] macro to
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
