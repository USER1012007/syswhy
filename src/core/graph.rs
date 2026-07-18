use std::collections::BTreeMap;

use crate::core::{Entity, EntityId, Relation};

#[derive(Debug, Clone, Default)]
pub struct EvidenceGraph {
    entities: BTreeMap<EntityId, Entity>,
    relations: Vec<Relation>,
}

impl EvidenceGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_entity(&mut self, entity: Entity) -> EntityId {
        let id = entity.id.clone();
        self.entities
            .entry(id.clone())
            .and_modify(|existing| {
                if existing.name.starts_with("PID ") && !entity.name.starts_with("PID ") {
                    existing.name = entity.name.clone();
                }
                existing.metadata.extend(entity.metadata.clone());
            })
            .or_insert(entity);
        id
    }

    pub fn add_relation(&mut self, relation: Relation) {
        if let Some(existing) = self.relations.iter_mut().find(|existing| {
            existing.from == relation.from
                && existing.to == relation.to
                && existing.kind == relation.kind
        }) {
            existing.confidence = existing.confidence.max(relation.confidence);

            for evidence in relation.evidence {
                if !existing.evidence.contains(&evidence) {
                    existing.evidence.push(evidence);
                }
            }

            return;
        }

        self.relations.push(relation);
    }

    pub fn entity(&self, id: &EntityId) -> Option<&Entity> {
        self.entities.get(id)
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn relations(&self) -> impl Iterator<Item = &Relation> {
        self.relations.iter()
    }

    pub fn outgoing(&self, id: &EntityId) -> impl Iterator<Item = &Relation> {
        self.relations
            .iter()
            .filter(move |relation| relation.from == *id)
    }

    pub fn incoming(&self, id: &EntityId) -> impl Iterator<Item = &Relation> {
        self.relations
            .iter()
            .filter(move |relation| relation.to == *id)
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty() && self.relations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{
        Confidence, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Relation, RelationKind,
    };

    #[test]
    fn adding_the_same_entity_twice_deduplicates_it() {
        let mut graph = EvidenceGraph::new();
        let entity = Entity::new(
            EntityId::new("file:/usr/bin/firefox"),
            EntityKind::Executable,
            "firefox",
        );

        graph.add_entity(entity.clone());
        graph.add_entity(entity);

        assert_eq!(graph.entity_count(), 1);
    }

    #[test]
    fn adding_same_entity_merges_metadata_and_improves_placeholder_name() {
        let mut graph = EvidenceGraph::new();
        let id = EntityId::new("process:42");
        graph.add_entity(Entity::new(id.clone(), EntityKind::Process, "PID 42"));

        let mut enriched = Entity::new(id.clone(), EntityKind::Process, "demo (42)");
        enriched
            .metadata
            .insert("cmdline".to_string(), "demo --flag".to_string());
        graph.add_entity(enriched);

        let entity = graph.entity(&id).unwrap();
        assert_eq!(entity.name, "demo (42)");
        assert_eq!(entity.metadata.get("cmdline").unwrap(), "demo --flag");
    }

    #[test]
    fn adding_the_same_relation_merges_evidence() {
        let mut graph = EvidenceGraph::new();
        let from = EntityId::new("file:/run/current-system/sw/bin/firefox");
        let to = EntityId::new("file:/nix/store/abc-firefox/bin/firefox");

        graph.add_relation(Relation::new(
            from.clone(),
            to.clone(),
            RelationKind::ResolvesTo,
            Confidence::Inferred,
            vec![Evidence::new(
                "filesystem",
                "PATH",
                "Executable was found in PATH",
                Confidence::Inferred,
            )],
        ));
        graph.add_relation(Relation::new(
            from,
            to,
            RelationKind::ResolvesTo,
            Confidence::Exact,
            vec![Evidence::new(
                "filesystem",
                "std::fs::canonicalize",
                "Symlink target was canonicalized",
                Confidence::Exact,
            )],
        ));

        let relation = graph.relations().next().unwrap();
        assert_eq!(graph.relation_count(), 1);
        assert_eq!(relation.confidence, Confidence::Exact);
        assert_eq!(relation.evidence.len(), 2);
    }
}
