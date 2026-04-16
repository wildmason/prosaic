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
//! In an `prosaic` engine, this powers the `{name|refer}` pipe's *Full form*
//! path. When multiple entities of the same type are known to the engine,
//! the algorithm adds distinguishing adjectives until the target is
//! unambiguous — producing "the domain class UserService" instead of the
//! bare "the class UserService" when an ambiguity-causing "the infra class
//! AuthService" has also been registered.

use crate::collections::HashMap;

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
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
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
    pub fn with_relation(mut self, label: impl Into<String>, target: impl Into<String>) -> Self {
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

/// Output of the graph-based REG algorithm.
///
/// Contains the attribute values chosen to premodify the head noun
/// (same as Dale & Reiter output), and an optional relation clause to
/// append as a postmodifier when attributes alone do not disambiguate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubgraphDescription {
    /// Attribute values to include as premodifiers, in preference order.
    /// May be empty when the entity is the only one of its type.
    pub attributes: Vec<String>,
    /// Distinguishing relation, if one was needed: `(label, target_name)`.
    ///
    /// The label is inserted verbatim into the surface form after the head
    /// noun — e.g. `Some(("that calls", "AuthService"))` renders as
    /// `"that calls AuthService"`.
    pub relation: Option<(String, String)>,
}

/// Shared core of both REG algorithms.
///
/// Runs the Dale & Reiter incremental attribute selection and returns
/// both the chosen attribute values AND the surviving distractor set after
/// those attributes have been applied. The surviving set is empty when
/// attributes alone disambiguate; non-empty when they do not.
///
/// Both [`distinguishing_attributes`] and [`distinguishing_subgraph`] call
/// this helper so the distractor-survival semantics are identical and
/// cannot diverge.
pub(crate) fn incremental_attributes_with_remaining<'a>(
    target: &EntityDescriptor,
    registry: &'a EntityRegistry,
    preference_order: &[String],
) -> (Vec<String>, Vec<&'a EntityDescriptor>) {
    // Build distractor set: all registry entries with the same type,
    // excluding the target itself.
    let mut distractors: Vec<&EntityDescriptor> = registry
        .iter()
        .filter(|d| d.name != target.name && d.entity_type == target.entity_type)
        .collect();

    // If nothing could confuse the target, no distinguishers needed.
    if distractors.is_empty() {
        return (Vec::new(), Vec::new());
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

    (chosen, distractors)
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
    let (attrs, _) = incremental_attributes_with_remaining(target, registry, preference_order);
    attrs
}

/// Krahmer et al. 2003 graph-based greedy REG algorithm.
///
/// Extends Dale & Reiter by also considering labeled directed relations
/// between entities. When attributes alone do not fully disambiguate the
/// target from all same-type distractors, the algorithm appends one
/// distinguishing relation clause.
///
/// Algorithm:
/// 1. Run D&R attribute selection (`incremental_attributes_with_remaining`).
/// 2. If no distractors survive the attribute filter, return early — no
///    relation needed (identical behaviour to D&R for this case).
/// 3. Walk the target's relations in insertion order. Pick the first
///    relation `(label, target_name)` that none of the surviving
///    distractors also hold.
/// 4. If all of the target's relations are shared with at least one
///    surviving distractor, return the best-effort attribute list with
///    no relation (may still be ambiguous — greedy fallback).
///
/// Complexity: O(|distractors| × (|attributes| + |relations|)) — linear
/// in registry size. No backtracking, no B&B.
pub fn distinguishing_subgraph(
    target: &EntityDescriptor,
    registry: &EntityRegistry,
    preference_order: &[String],
) -> SubgraphDescription {
    let (attrs, remaining) =
        incremental_attributes_with_remaining(target, registry, preference_order);

    // Attributes alone were sufficient — no relation needed.
    if remaining.is_empty() {
        return SubgraphDescription {
            attributes: attrs,
            relation: None,
        };
    }

    // Walk target's relations in insertion order. Pick the first relation
    // that no surviving distractor also holds.
    for (label, target_name) in &target.relations {
        let any_shared = remaining.iter().any(|d| {
            d.relations
                .iter()
                .any(|(l, t)| l == label && t == target_name)
        });
        if !any_shared {
            return SubgraphDescription {
                attributes: attrs,
                relation: Some((label.clone(), target_name.clone())),
            };
        }
    }

    // Exhausted — return best-effort (may still be ambiguous).
    SubgraphDescription {
        attributes: attrs,
        relation: None,
    }
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
        let target =
            EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain");
        let registry = reg_with(vec![target.clone()]);
        let attrs = distinguishing_attributes(&target, &registry, &[]);
        assert!(attrs.is_empty());
    }

    #[test]
    fn different_type_distractor_does_not_force_attribute() {
        let target =
            EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain");
        let other = EntityDescriptor::new("UserService", "trait").with_attribute("layer", "infra");
        let registry = reg_with(vec![target.clone(), other]);
        // Same name but different type — the head noun alone disambiguates.
        let attrs = distinguishing_attributes(&target, &registry, &[]);
        assert!(attrs.is_empty());
    }

    #[test]
    fn same_type_requires_distinguishing_attribute() {
        let target =
            EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain");
        let distractor =
            EntityDescriptor::new("AuthService", "class").with_attribute("layer", "infra");
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
            &["color".to_string(), "size".to_string(), "shape".to_string()],
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
        let target = EntityDescriptor::new("Foo", "widget").with_attribute("size", "small");
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
        let e = EntityDescriptor::new("Handler", "function").with_relation("calls", "AuthService");
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

    // ── distinguishing_subgraph (graph-based REG) ─────────────────────────

    #[test]
    fn graph_reg_no_distractors_returns_empty() {
        let target = EntityDescriptor::new("Foo", "class");
        let registry = reg_with(vec![target.clone()]);
        let desc = distinguishing_subgraph(&target, &registry, &[]);
        assert!(desc.attributes.is_empty());
        assert!(desc.relation.is_none());
    }

    #[test]
    fn graph_reg_falls_back_to_dale_reiter_when_attributes_suffice() {
        let target =
            EntityDescriptor::new("UserService", "class").with_attribute("layer", "domain");
        let other = EntityDescriptor::new("AuthService", "class").with_attribute("layer", "infra");
        let registry = reg_with(vec![target.clone(), other]);
        let desc = distinguishing_subgraph(&target, &registry, &[]);
        assert_eq!(desc.attributes, vec!["domain".to_string()]);
        assert!(desc.relation.is_none());
    }

    #[test]
    fn graph_reg_adds_relation_when_attributes_dont_disambiguate() {
        // Two handlers, both in the same layer, differentiated only by
        // what they call.
        let target = EntityDescriptor::new("LoginHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("calls", "AuthService");
        let other = EntityDescriptor::new("LogoutHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("calls", "SessionService");
        let registry = reg_with(vec![target.clone(), other]);
        let desc = distinguishing_subgraph(&target, &registry, &[]);
        // Attributes alone don't distinguish (both are `api` layer), so
        // the relation must be included.
        assert_eq!(
            desc.relation,
            Some(("calls".to_string(), "AuthService".to_string()))
        );
    }

    #[test]
    fn graph_reg_skips_shared_relation_picks_next() {
        // Two handlers, both call the same logging service; but target has
        // an additional distinguishing relation.
        let target = EntityDescriptor::new("LoginHandler", "function")
            .with_relation("calls", "LogService")
            .with_relation("tests", "LoginTests");
        let other = EntityDescriptor::new("LogoutHandler", "function")
            .with_relation("calls", "LogService")
            .with_relation("tests", "LogoutTests");
        let registry = reg_with(vec![target.clone(), other]);
        let desc = distinguishing_subgraph(&target, &registry, &[]);
        assert_eq!(
            desc.relation,
            Some(("tests".to_string(), "LoginTests".to_string()))
        );
    }

    #[test]
    fn graph_reg_gives_up_when_nothing_distinguishes() {
        // Two identical entities — no attributes, no distinguishing
        // relations.
        let target = EntityDescriptor::new("Foo", "thing").with_relation("calls", "X");
        let other = EntityDescriptor::new("Bar", "thing").with_relation("calls", "X");
        let registry = reg_with(vec![target.clone(), other]);
        let desc = distinguishing_subgraph(&target, &registry, &[]);
        // Returns whatever attributes D&R could find (none here) and no
        // relation — greedy fallback.
        assert!(desc.relation.is_none());
    }

    #[test]
    fn graph_reg_combines_attributes_and_relation() {
        // Three handlers:
        // - Target:      layer=api, calls=AuthService
        // - Distractor1: layer=api, calls=SessionService (differs by relation)
        // - Distractor2: layer=web, calls=AuthService    (differs by attribute)
        //
        // D&R picks `layer=api` to exclude distractor2, leaving distractor1.
        // Graph-based adds `calls=AuthService` to exclude distractor1.
        let target = EntityDescriptor::new("LoginHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("calls", "AuthService");
        let d1 = EntityDescriptor::new("LogoutHandler", "function")
            .with_attribute("layer", "api")
            .with_relation("calls", "SessionService");
        let d2 = EntityDescriptor::new("ProfileHandler", "function")
            .with_attribute("layer", "web")
            .with_relation("calls", "AuthService");
        let registry = reg_with(vec![target.clone(), d1, d2]);
        let desc = distinguishing_subgraph(&target, &registry, &[]);
        assert_eq!(desc.attributes, vec!["api".to_string()]);
        assert_eq!(
            desc.relation,
            Some(("calls".to_string(), "AuthService".to_string()))
        );
    }
}
