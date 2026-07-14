use std::path::{Path, PathBuf};

use crate::backend::{Backend, BackendError, BackendOutput, SystemContext};
use crate::core::{
    Confidence, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Query, Relation,
    RelationKind,
};

#[derive(Debug, Clone, Default)]
pub struct NixBackend;

impl NixBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for NixBackend {
    fn name(&self) -> &'static str {
        "nix"
    }

    fn detect(&self, context: &SystemContext) -> bool {
        context.has_nix_store
    }

    fn supports(&self, _query: &Query) -> bool {
        true
    }

    fn investigate(
        &self,
        _query: &Query,
        graph: &mut EvidenceGraph,
    ) -> Result<BackendOutput, BackendError> {
        let path_entities: Vec<_> = graph
            .entities()
            .filter(|entity| matches!(entity.kind, EntityKind::File | EntityKind::Executable))
            .filter(|entity| {
                !graph
                    .outgoing(&entity.id)
                    .any(|relation| relation.kind == RelationKind::ResolvesTo)
            })
            .map(|entity| (entity.id.clone(), PathBuf::from(&entity.name)))
            .collect();

        let mut enriched = false;
        for (entity_id, path) in path_entities {
            if let Some(store_path) = containing_store_path(&path) {
                add_store_path_relation(graph, entity_id, store_path)?;
                enriched = true;
            }
        }

        if enriched {
            Ok(BackendOutput::new())
        } else {
            Ok(BackendOutput::new()
                .with_incomplete("nix did not find any /nix/store paths in the current graph"))
        }
    }
}

fn add_store_path_relation(
    graph: &mut EvidenceGraph,
    file_id: EntityId,
    store_path: PathBuf,
) -> Result<(), BackendError> {
    let store_path = display_path(&store_path)?;
    let store_id = EntityId::new(format!("store-path:{store_path}"));
    graph.add_entity(Entity::new(
        store_id.clone(),
        EntityKind::StorePath,
        store_path,
    ));
    graph.add_relation(Relation::new(
        file_id,
        store_id,
        RelationKind::BelongsTo,
        Confidence::Exact,
        vec![Evidence::new(
            "nix",
            "/nix/store path detection",
            "Path is contained by this Nix store path",
            Confidence::Exact,
        )],
    ));

    Ok(())
}

fn containing_store_path(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();

    let root = components.next()?;
    if root.as_os_str() != "/" {
        return None;
    }

    if components.next()?.as_os_str() != "nix" {
        return None;
    }

    if components.next()?.as_os_str() != "store" {
        return None;
    }

    let store_name = components.next()?;
    Some(Path::new("/nix/store").join(store_name.as_os_str()))
}

fn display_path(path: &Path) -> Result<String, BackendError> {
    if path.as_os_str().is_empty() {
        return Err(BackendError::Failed("empty Nix store path".to_string()));
    }

    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::backend::Backend;
    use crate::backend::nix::{NixBackend, containing_store_path};
    use crate::core::{
        Confidence, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Query, Relation,
        RelationKind,
    };

    #[test]
    fn detects_containing_store_path() {
        assert_eq!(
            containing_store_path(Path::new("/nix/store/abc123-firefox-140.0/bin/firefox"))
                .unwrap(),
            Path::new("/nix/store/abc123-firefox-140.0")
        );
    }

    #[test]
    fn ignores_non_store_paths() {
        assert!(containing_store_path(Path::new("/usr/bin/bash")).is_none());
    }

    #[test]
    fn adds_belongs_to_relation_for_store_path_entities() {
        let mut graph = EvidenceGraph::new();
        graph.add_entity(Entity::new(
            EntityId::new("file:/nix/store/abc123-firefox-140.0/bin/firefox"),
            EntityKind::Executable,
            "/nix/store/abc123-firefox-140.0/bin/firefox",
        ));

        let output = NixBackend::new()
            .investigate(&Query::Auto("firefox".to_string()), &mut graph)
            .unwrap();

        assert!(output.incomplete.is_empty());
        assert_eq!(graph.entity_count(), 2);
        assert_eq!(graph.relation_count(), 1);
        assert_eq!(
            graph.relations().next().unwrap().kind,
            RelationKind::BelongsTo
        );
    }

    #[test]
    fn prefers_canonical_target_over_resolving_symlink() {
        let mut graph = EvidenceGraph::new();
        let link_id = graph.add_entity(Entity::new(
            EntityId::new("file:/nix/store/abc123-bash/bin/sh"),
            EntityKind::Executable,
            "/nix/store/abc123-bash/bin/sh",
        ));
        let target_id = graph.add_entity(Entity::new(
            EntityId::new("file:/nix/store/abc123-bash/bin/bash"),
            EntityKind::File,
            "/nix/store/abc123-bash/bin/bash",
        ));
        graph.add_relation(Relation::new(
            link_id,
            target_id.clone(),
            RelationKind::ResolvesTo,
            Confidence::Exact,
            vec![Evidence::new(
                "filesystem",
                "std::fs::canonicalize",
                "Path resolves to this canonical target",
                Confidence::Exact,
            )],
        ));

        NixBackend::new()
            .investigate(&Query::Auto("sh".to_string()), &mut graph)
            .unwrap();

        let belongs_to = graph
            .relations()
            .filter(|relation| relation.kind == RelationKind::BelongsTo)
            .collect::<Vec<_>>();
        assert_eq!(belongs_to.len(), 1);
        assert_eq!(belongs_to[0].from, target_id);
    }
}
