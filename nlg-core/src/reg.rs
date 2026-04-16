//! Referring Expression Generation (REG).
//!
//! Implements two algorithms:
//!
//! - **Dale & Reiter Incremental Algorithm** (1995): given a target entity and
//!   a distractor set, select the shortest set of unary attributes that
//!   uniquely identifies the target.
//!
//! - **Krahmer et al. Graph-Based Greedy Algorithm** (2003): extends D&R by
//!   also considering labeled directed relations between entities. When
//!   attributes alone do not disambiguate, the algorithm appends one
//!   relation clause (e.g. "that calls AuthService") to the referring
//!   expression.
//!
//! In an `nlg` engine, this powers the `{name|refer}` pipe's *Full form*
//! path. When multiple entities of the same type are known to the engine,
//! the algorithm adds distinguishing adjectives until the target is
//! unambiguous — producing "the domain class UserService" instead of the
//! bare "the class UserService" when an ambiguity-causing "the infra class
//! AuthService" has also been registered.

use std::collections::HashMap;

/// A described entity. Attributes are intentionally ordered so the default
/// preference ordering respects registration order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityDescriptor {
    pub name: String,
    pub entity_type: String,
    pub attributes: Vec<(String, String)>,
    /// Labeled directed edges from this entity to other named entities.
    ///
    /// Each `(relation_label, target_name)` pair. The label is the
    /// surface-form fragment that will be inserted verbatim after the head
    /// noun in the referring expression — e.g. `("that calls", "AuthService")`
    /// renders as `"that calls AuthService"`. Labels are chosen by the caller
    /// to read naturally in context.
    ///
    /// `#[serde(default)]` ensures existing serialized `EntityDescriptor`
    /// payloads that lack this field still deserialize cleanly.
    #[cfg_attr(feature = "serde", serde(default))]
    pub relations: Vec<(String, String)>,
}

impl EntityDescriptor {
    pub fn new(name: impl Into<String>, entity_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entity_type: entity_type.into(),
            attributes: Vec::new(),
            relations: Vec::new(),
        }
    }

    /// Append an attribute. Chainable.
    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.attributes.push((key.into(), value.into()));
        self
    }

    /// Append a labeled directed relation to another entity. Chainable.
    ///
    /// `label` is the surface-form fragment inserted verbatim after the head
    /// noun when this relation is selected by the graph-based algorithm
    /// (e.g. `"that calls"`). `target` is the name of the target entity.
    ///
    /// Relations are considered in insertion order during REG.
    pub fn with_relation(
        mut self,
        label: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        self.relations.push((label.into(), target.into()));
        self
    }

    /// Look up an attribute by key.
    pub fn attribute(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Look up a relation's target by label.
    ///
    /// Returns the target name of the first relation whose label matches, or
    /// `None` if no such relation exists.
    pub fn relation(&self, label: &str) -> Option<&str> {
        self.relations
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, t)| t.as_str())
    }
}

/// Registry of descriptors for entities known to the engine.
///
/// Keyed by the tuple `(entity_type, name)` so the same name can represent
/// distinct entities of different types (e.g. `UserService` as a `class`
/// and `UserService` as a `trait` are independent entries). Callers whose
/// entities collide on both type and name should use fully-qualified names
/// (e.g. `"auth::init"`) to disambiguate.
#[derive(Debug, Clone, Default)]
pub struct EntityRegistry {
    entries: HashMap<(String, String), EntityDescriptor>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace an entity. Last write wins for a given
    /// `(entity_type, name)` pair — different-type entries with the same
    /// name coexist.
    pub fn insert(&mut self, descriptor: EntityDescriptor) {
        let key = (descriptor.entity_type.clone(), descriptor.name.clone());
        self.entries.insert(key, descriptor);
    }

    /// Look up by `(entity_type, name)` — the canonical registry key.
    pub fn get(&self, entity_type: &str, name: &str) -> Option<&EntityDescriptor> {
        self.entries
            .get(&(entity_type.to_string(), name.to_string()))
    }

    pub fn iter(&self) -> impl Iterator<Item = &EntityDescriptor> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Dale & Reiter's Incremental Algorithm.
///
/// Returns the ordered list of attribute values that should premodify the
/// head noun, in the order they should appear. The head noun (entity type)
/// is always included implicitly — callers render it alongside the name
/// themselves: `"the <attrs joined by space> <type> <name>"`.
///
/// Algorithm:
/// 1. Filter distractors to those sharing the target's entity type (the
///    type is always the head noun, so same-type entities are the only
///    candidates that could still be confused).
/// 2. Walk attributes in preference order. For each attribute the target
///    has, include it if it rules out at least one remaining distractor.
/// 3. Stop when no distractors remain.
/// 4. If the attribute list is exhausted with distractors still present,
///    return whatever was chosen — the result may still be ambiguous but
///    uses all available discriminating information.
pub fn distinguishing_attributes(
    target: &EntityDescriptor,
    registry: &EntityRegistry,
    preference_order: &[String],
) -> Vec<String> {
    // Build distractor set: all registry entries with the same type,
    // excluding the target itself.
    let mut distractors: Vec<&EntityDescriptor> = registry
        .iter()
        .filter(|d| d.name != target.name && d.entity_type == target.entity_type)
        .collect();

    // If nothing could confuse the target, no distinguishers needed.
    if distractors.is_empty() {
        return Vec::new();
    }

    // Determine the attribute walk order: explicit preference first, then
    // the target's own registration order (so any not covered by the
    // preference list still get a chance).
    let mut walked: Vec<&String> = preference_order.iter().collect();
    for (k, _) in &target.attributes {
        if !walked.iter().any(|s| s.as_str() == k.as_str()) {
            walked.push(k);
        }
    }

    let mut chosen: Vec<String> = Vec::new();

    for attr_key in walked {
        if distractors.is_empty() {
            break;
        }
        let target_value = match target.attribute(attr_key) {
            Some(v) => v,
            None => continue,
        };

        // Would this attribute rule out any distractor?
        let still_matching: Vec<&EntityDescriptor> = distractors
            .iter()
            .copied()
            .filter(|d| d.attribute(attr_key) == Some(target_value))
            .collect();

        if still_matching.len() < distractors.len() {
            chosen.push(target_value.to_string());
            distractors = still_matching;
        }
    }

    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg_with(entities: Vec<EntityDescriptor>) -> EntityRegistry {
        let mut r = EntityRegistry::new();
        for e in entities {
            r.insert(e);
        }
        r
    }

    #[test]
    fn no_distractors_yields_empty_attribute_list() {
        let target = EntityDescriptor::new("UserService", "class")
            .with_attribute("layer", "domain");
        let registry = reg_with(vec![target.clone()]);
        let attrs = distinguishing_attributes(&target, &registry, &[]);
        assert!(attrs.is_empty());
    }

    #[test]
    fn different_type_distractor_does_not_force_attribute() {
        let target = EntityDescriptor::new("UserService", "class")
            .with_attribute("layer", "domain");
        let other = EntityDescriptor::new("UserService", "trait")
            .with_attribute("layer", "infra");
        let registry = reg_with(vec![target.clone(), other]);
        // Same name but different type — the head noun alone disambiguates.
        let attrs = distinguishing_attributes(&target, &registry, &[]);
        assert!(attrs.is_empty());
    }

    #[test]
    fn same_type_requires_distinguishing_attribute() {
        let target = EntityDescriptor::new("UserService", "class")
            .with_attribute("layer", "domain");
        let distractor = EntityDescriptor::new("AuthService", "class")
            .with_attribute("layer", "infra");
        let registry = reg_with(vec![target.clone(), distractor]);
        let attrs = distinguishing_attributes(&target, &registry, &[]);
        assert_eq!(attrs, vec!["domain".to_string()]);
    }

    #[test]
    fn preference_order_is_respected() {
        let target = EntityDescriptor::new("Foo", "class")
            .with_attribute("color", "red")
            .with_attribute("size", "small");
        let d1 = EntityDescriptor::new("Bar", "class")
            .with_attribute("color", "blue")
            .with_attribute("size", "small");

        let registry = reg_with(vec![target.clone(), d1]);

        // With size preferred first, size is useless (both small) so color is used.
        let attrs = distinguishing_attributes(
            &target,
            &registry,
            &["size".to_string(), "color".to_string()],
        );
        assert_eq!(attrs, vec!["red".to_string()]);
    }

    #[test]
    fn preferred_attribute_selected_when_sufficient() {
        let target = EntityDescriptor::new("Foo", "widget")
            .with_attribute("color", "red")
            .with_attribute("size", "small");
        let d1 = EntityDescriptor::new("Bar", "widget")
            .with_attribute("color", "blue")
            .with_attribute("size", "large");

        let registry = reg_with(vec![target.clone(), d1]);
        let attrs = distinguishing_attributes(&target, &registry, &["color".to_string()]);
        assert_eq!(attrs, vec!["red".to_string()]);
    }

    #[test]
    fn multiple_attributes_needed_for_full_disambiguation() {
        let target = EntityDescriptor::new("Foo", "widget")
            .with_attribute("color", "red")
            .with_attribute("size", "small");
        let same_color = EntityDescriptor::new("Bar", "widget")
            .with_attribute("color", "red")
            .with_attribute("size", "large");
        let same_size = EntityDescriptor::new("Baz", "widget")
            .with_attribute("color", "blue")
            .with_attribute("size", "small");

        let registry = reg_with(vec![target.clone(), same_color, same_size]);
        let attrs = distinguishing_attributes(
            &target,
            &registry,
            &["color".to_string(), "size".to_string()],
        );
        // color removes same_size (blue ≠ red); size then removes same_color (large ≠ small).
        assert_eq!(attrs, vec!["red".to_string(), "small".to_string()]);
    }

    #[test]
    fn useless_attribute_is_skipped() {
        // All three widgets are red — color doesn't disambiguate.
        let target = EntityDescriptor::new("Foo", "widget")
            .with_attribute("color", "red")
            .with_attribute("size", "small");
        let d1 = EntityDescriptor::new("Bar", "widget")
            .with_attribute("color", "red")
            .with_attribute("size", "large");
        let d2 = EntityDescriptor::new("Baz", "widget")
            .with_attribute("color", "red")
            .with_attribute("size", "medium");

        let registry = reg_with(vec![target.clone(), d1, d2]);
        let attrs = distinguishing_attributes(
            &target,
            &registry,
            &["color".to_string(), "size".to_string()],
        );
        // color does nothing (all red), size alone distinguishes (small vs large, medium).
        assert_eq!(attrs, vec!["small".to_string()]);
    }

    #[test]
    fn stops_as_soon_as_unambiguous() {
        let target = EntityDescriptor::new("Foo", "widget")
            .with_attribute("color", "red")
            .with_attribute("size", "small")
            .with_attribute("shape", "round");
        let d1 = EntityDescriptor::new("Bar", "widget").with_attribute("color", "blue");

        let registry = reg_with(vec![target.clone(), d1]);
        let attrs = distinguishing_attributes(
            &target,
            &registry,
            &[
                "color".to_string(),
                "size".to_string(),
                "shape".to_string(),
            ],
        );
        // color alone is enough; size and shape must not be included.
        assert_eq!(attrs, vec!["red".to_string()]);
    }

    #[test]
    fn falls_back_to_registration_order_when_no_preference() {
        let target = EntityDescriptor::new("Foo", "widget")
            .with_attribute("first_attr", "A")
            .with_attribute("second_attr", "B");
        let d1 = EntityDescriptor::new("Bar", "widget")
            .with_attribute("first_attr", "X")
            .with_attribute("second_attr", "B");

        let registry = reg_with(vec![target.clone(), d1]);
        // With no preference order, registration order picks first_attr,
        // which is sufficient.
        let attrs = distinguishing_attributes(&target, &registry, &[]);
        assert_eq!(attrs, vec!["A".to_string()]);
    }

    #[test]
    fn missing_attribute_on_target_skips_without_panic() {
        let target = EntityDescriptor::new("Foo", "widget")
            .with_attribute("size", "small");
        let d1 = EntityDescriptor::new("Bar", "widget").with_attribute("size", "large");

        let registry = reg_with(vec![target.clone(), d1]);
        // Preference includes "color", which the target doesn't have; skip and try "size".
        let attrs = distinguishing_attributes(
            &target,
            &registry,
            &["color".to_string(), "size".to_string()],
        );
        assert_eq!(attrs, vec!["small".to_string()]);
    }

    #[test]
    fn registry_insert_replaces_same_type_and_name() {
        let mut r = EntityRegistry::new();
        r.insert(EntityDescriptor::new("X", "t").with_attribute("a", "1"));
        r.insert(EntityDescriptor::new("X", "t").with_attribute("a", "2"));
        assert_eq!(r.get("t", "X").unwrap().attribute("a"), Some("2"));
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn registry_keeps_same_name_different_type_as_separate_entries() {
        let mut r = EntityRegistry::new();
        r.insert(EntityDescriptor::new("UserService", "class").with_attribute("a", "1"));
        r.insert(EntityDescriptor::new("UserService", "trait").with_attribute("a", "2"));
        assert_eq!(r.len(), 2);
        assert_eq!(
            r.get("class", "UserService").unwrap().attribute("a"),
            Some("1")
        );
        assert_eq!(
            r.get("trait", "UserService").unwrap().attribute("a"),
            Some("2")
        );
    }

    // ── Relations on EntityDescriptor ────────────────────────────────────────

    #[test]
    fn with_relation_adds_edge() {
        let e = EntityDescriptor::new("Handler", "function")
            .with_relation("calls", "AuthService");
        assert_eq!(
            e.relations,
            vec![("calls".to_string(), "AuthService".to_string())]
        );
    }

    #[test]
    fn relation_lookup_by_label() {
        let e = EntityDescriptor::new("Handler", "function")
            .with_relation("calls", "AuthService")
            .with_relation("tests", "HandlerTests");
        assert_eq!(e.relation("calls"), Some("AuthService"));
        assert_eq!(e.relation("tests"), Some("HandlerTests"));
        assert_eq!(e.relation("unknown"), None);
    }

    #[test]
    fn default_has_empty_relations() {
        let e = EntityDescriptor::default();
        assert!(e.relations.is_empty());
    }
}
