use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::backend::{Backend, BackendError, BackendOutput, SystemContext};
use crate::core::{
    Confidence, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Query, Relation,
    RelationKind,
};

#[derive(Debug, Clone)]
pub struct FileSystemBackend {
    path_env: Option<String>,
}

impl FileSystemBackend {
    pub fn from_context(context: &SystemContext) -> Self {
        Self {
            path_env: context.path_env.clone(),
        }
    }

    pub fn with_path_env(path_env: impl Into<String>) -> Self {
        Self {
            path_env: Some(path_env.into()),
        }
    }

    fn investigate_auto(
        &self,
        name: &str,
        graph: &mut EvidenceGraph,
    ) -> Result<BackendOutput, BackendError> {
        let Some(path_env) = &self.path_env else {
            return Ok(BackendOutput::new()
                .with_incomplete("filesystem could not search PATH because PATH is unset"));
        };

        let Some(path) = find_executable(name, path_env) else {
            return Ok(BackendOutput::new().with_incomplete(format!(
                "filesystem could not find executable {name:?} in PATH"
            )));
        };

        let entity_id = add_path_entity(graph, &path, EntityKind::Executable)?;
        add_canonical_relation(graph, &entity_id, &path)?;

        Ok(BackendOutput::new().with_match(entity_id))
    }

    fn investigate_file(
        &self,
        path: &Path,
        graph: &mut EvidenceGraph,
    ) -> Result<BackendOutput, BackendError> {
        if !path.exists() {
            return Ok(BackendOutput::new()
                .with_incomplete(format!("filesystem could not find path {}", path.display())));
        }

        let kind = if is_executable(path) {
            EntityKind::Executable
        } else {
            EntityKind::File
        };
        let entity_id = add_path_entity(graph, path, kind)?;
        add_canonical_relation(graph, &entity_id, path)?;

        Ok(BackendOutput::new().with_match(entity_id))
    }

    fn enrich_existing_paths(&self, graph: &mut EvidenceGraph) -> BackendOutput {
        let path_entities = graph
            .entities()
            .filter(|entity| matches!(entity.kind, EntityKind::File | EntityKind::Executable))
            .filter(|entity| {
                !graph
                    .outgoing(&entity.id)
                    .any(|relation| relation.kind == RelationKind::ResolvesTo)
            })
            .map(|entity| (entity.id.clone(), PathBuf::from(&entity.name)))
            .collect::<Vec<_>>();

        let mut output = BackendOutput::new();
        for (entity_id, path) in path_entities {
            if !path.exists() {
                continue;
            }

            if let Err(error) = add_canonical_relation(graph, &entity_id, &path) {
                output.incomplete.push(format!(
                    "filesystem could not resolve {}: {error:?}",
                    path.display()
                ));
            }
        }

        output
    }
}

impl Backend for FileSystemBackend {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    fn detect(&self, _context: &SystemContext) -> bool {
        true
    }

    fn supports(&self, query: &Query) -> bool {
        matches!(
            query,
            Query::Auto(_) | Query::File(_) | Query::Process(_) | Query::Service(_)
        )
    }

    fn investigate(
        &self,
        query: &Query,
        graph: &mut EvidenceGraph,
    ) -> Result<BackendOutput, BackendError> {
        match query {
            Query::Auto(name) => self.investigate_auto(name, graph),
            Query::File(path) => self.investigate_file(path, graph),
            Query::Process(_) | Query::Service(_) => Ok(self.enrich_existing_paths(graph)),
            _ => Err(BackendError::UnsupportedQuery),
        }
    }
}

fn find_executable(name: &str, path_env: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let path = PathBuf::from(name);
        return (path.is_file() && is_executable(&path)).then_some(path);
    }

    path_env
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(|entry| Path::new(entry).join(name))
        .find(|candidate| candidate.is_file() && is_executable(candidate))
}

fn add_path_entity(
    graph: &mut EvidenceGraph,
    path: &Path,
    kind: EntityKind,
) -> Result<EntityId, BackendError> {
    let path = display_path(path)?;
    let id = EntityId::new(format!("file:{path}"));
    let entity = Entity::new(id.clone(), kind, path);
    Ok(graph.add_entity(entity))
}

fn add_canonical_relation(
    graph: &mut EvidenceGraph,
    from_id: &EntityId,
    path: &Path,
) -> Result<(), BackendError> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        BackendError::Failed(format!(
            "failed to canonicalize {}: {error}",
            path.display()
        ))
    })?;

    if canonical == path {
        return Ok(());
    }

    let canonical_id = add_path_entity(graph, &canonical, EntityKind::File)?;
    graph.add_relation(Relation::new(
        from_id.clone(),
        canonical_id,
        RelationKind::ResolvesTo,
        Confidence::Exact,
        vec![Evidence::new(
            "filesystem",
            "std::fs::canonicalize",
            "Path resolves to this canonical target",
            Confidence::Exact,
        )],
    ));

    Ok(())
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn display_path(path: &Path) -> Result<String, BackendError> {
    if path.as_os_str().is_empty() {
        return Err(BackendError::Failed("empty filesystem path".to_string()));
    }

    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::backend::Backend;
    use crate::backend::filesystem::FileSystemBackend;
    use crate::core::{Entity, EntityId, EntityKind, EvidenceGraph, Query, RelationKind};

    #[test]
    fn auto_query_finds_executable_in_path() {
        let fixture = TempFixture::new();
        let executable = fixture.path.join("demo");
        fs::write(&executable, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

        let backend = FileSystemBackend::with_path_env(fixture.path.display().to_string());
        let mut graph = EvidenceGraph::new();
        let output = backend
            .investigate(&Query::Auto("demo".to_string()), &mut graph)
            .unwrap();

        assert_eq!(output.matches.len(), 1);
        assert_eq!(graph.entity_count(), 1);
    }

    #[test]
    fn file_query_records_symlink_resolution() {
        let fixture = TempFixture::new();
        let target = fixture.path.join("target");
        let link = fixture.path.join("link");
        fs::write(&target, "target\n").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let backend = FileSystemBackend::with_path_env("");
        let mut graph = EvidenceGraph::new();
        let output = backend.investigate(&Query::File(link), &mut graph).unwrap();

        assert_eq!(output.matches.len(), 1);
        assert_eq!(graph.relation_count(), 1);
        assert_eq!(
            graph.relations().next().unwrap().kind,
            RelationKind::ResolvesTo
        );
    }

    #[test]
    fn service_query_enriches_existing_file_entities() {
        let fixture = TempFixture::new();
        let target = fixture.path.join("target.service");
        let link = fixture.path.join("linked.service");
        fs::write(&target, "[Service]\n").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let backend = FileSystemBackend::with_path_env("");
        let mut graph = EvidenceGraph::new();
        graph.add_entity(Entity::new(
            EntityId::new(format!("file:{}", link.display())),
            EntityKind::File,
            link.display().to_string(),
        ));

        let output = backend
            .investigate(&Query::Service("linked".to_string()), &mut graph)
            .unwrap();

        assert!(output.matches.is_empty());
        assert_eq!(graph.relation_count(), 1);
        assert_eq!(
            graph.relations().next().unwrap().kind,
            RelationKind::ResolvesTo
        );
    }

    struct TempFixture {
        path: PathBuf,
    }

    impl TempFixture {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "syswhy-filesystem-test-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempFixture {
        fn drop(&mut self) {
            remove_dir_all_best_effort(&self.path);
        }
    }

    fn remove_dir_all_best_effort(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }
}
