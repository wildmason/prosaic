//! Rhetorical Structure Theory relations for discourse labeling.

/// A rhetorical relation between a discourse unit and its predecessor.
///
/// See Mann & Thompson (1988). The subset below covers the eight most-common
/// relations in transaction/change-report narratives. When an RST relation
/// is attached to an event in a [`crate::DocumentPlan`], the renderer uses
/// the relation to pick a discourse marker ("Furthermore", "However",
/// "As a result", etc.) instead of the default inter-sentence space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RstRelation {
    /// This unit adds detail to the previous unit.
    Elaboration,
    /// This unit contrasts with the previous unit.
    Contrast,
    /// This unit is caused by the previous unit.
    Cause,
    /// This unit is a result of the previous unit.
    Result,
    /// This unit runs counter to an expectation set up by the previous unit.
    Concession,
    /// This unit follows the previous unit in time.
    Sequence,
    /// This unit is conditional on the previous unit.
    Condition,
    /// This unit provides context for the previous unit.
    Background,
    /// This unit summarizes the preceding units.
    Summary,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_are_distinct() {
        assert_ne!(RstRelation::Elaboration, RstRelation::Contrast);
        assert_ne!(RstRelation::Cause, RstRelation::Result);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip() {
        let rel = RstRelation::Elaboration;
        let json = serde_json::to_string(&rel).unwrap();
        let back: RstRelation = serde_json::from_str(&json).unwrap();
        assert_eq!(rel, back);
    }
}
