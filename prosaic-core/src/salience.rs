use crate::context::{Context, Value};

/// Salience level of an event — how much detail/emphasis it deserves.
///
/// Templates can be registered for specific salience levels, and the engine
/// selects the appropriate level based on event magnitude.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Salience {
    /// Minor changes — single-consumer or no-impact. Brief, often parenthetical.
    Low,
    /// Standard changes — the default verbosity level.
    #[default]
    Medium,
    /// Major changes — high impact, deserves elaboration.
    High,
}

/// Thresholds for automatic salience derivation from context.
#[derive(Debug, Clone, Copy)]
pub struct SalienceThresholds {
    /// consumer_count below this is Low (exclusive).
    pub low_max: i64,
    /// consumer_count at or above this is High.
    pub high_min: i64,
}

impl Default for SalienceThresholds {
    fn default() -> Self {
        Self {
            low_max: 2,   // 0, 1 → Low
            high_min: 20, // 20+ → High; 2-19 → Medium
        }
    }
}

impl Salience {
    /// Derive salience from a rendering context.
    ///
    /// Order of precedence:
    /// 1. Explicit `salience` key in context (with value "low"/"medium"/"high")
    /// 2. `consumer_count` mapped through thresholds
    /// 3. Default (Medium)
    pub fn from_context(ctx: &Context, thresholds: SalienceThresholds) -> Self {
        // Explicit override
        if let Some(Value::String(s)) = ctx.get("salience") {
            match s.to_lowercase().as_str() {
                "low" => return Salience::Low,
                "medium" => return Salience::Medium,
                "high" => return Salience::High,
                _ => {}
            }
        }

        // Derive from consumer_count
        if let Some(Value::Number(n)) = ctx.get("consumer_count") {
            if *n < thresholds.low_max {
                return Salience::Low;
            }
            if *n >= thresholds.high_min {
                return Salience::High;
            }
            return Salience::Medium;
        }

        Salience::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with(key: &str, value: Value) -> Context {
        let mut c = Context::new();
        c.insert(key, value);
        c
    }

    #[test]
    fn explicit_salience_overrides_count() {
        let mut c = Context::new();
        c.insert("consumer_count", Value::Number(100));
        c.insert("salience", Value::String("low".into()));

        assert_eq!(
            Salience::from_context(&c, SalienceThresholds::default()),
            Salience::Low
        );
    }

    #[test]
    fn derives_low_from_small_count() {
        let c = ctx_with("consumer_count", Value::Number(1));
        assert_eq!(
            Salience::from_context(&c, SalienceThresholds::default()),
            Salience::Low
        );
    }

    #[test]
    fn derives_high_from_large_count() {
        let c = ctx_with("consumer_count", Value::Number(25));
        assert_eq!(
            Salience::from_context(&c, SalienceThresholds::default()),
            Salience::High
        );
    }

    #[test]
    fn derives_medium_from_middle_count() {
        let c = ctx_with("consumer_count", Value::Number(5));
        assert_eq!(
            Salience::from_context(&c, SalienceThresholds::default()),
            Salience::Medium
        );
    }

    #[test]
    fn defaults_to_medium_without_count() {
        let c = Context::new();
        assert_eq!(
            Salience::from_context(&c, SalienceThresholds::default()),
            Salience::Medium
        );
    }

    #[test]
    fn zero_count_is_low() {
        let c = ctx_with("consumer_count", Value::Number(0));
        assert_eq!(
            Salience::from_context(&c, SalienceThresholds::default()),
            Salience::Low
        );
    }

    #[test]
    fn unknown_salience_string_falls_back() {
        let mut c = Context::new();
        c.insert("consumer_count", Value::Number(5));
        c.insert("salience", Value::String("bogus".into()));
        assert_eq!(
            Salience::from_context(&c, SalienceThresholds::default()),
            Salience::Medium
        );
    }
}
