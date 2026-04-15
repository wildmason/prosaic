use std::collections::HashMap;

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
    values: HashMap<String, Value>,
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
