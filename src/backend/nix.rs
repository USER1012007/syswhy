use std::path::{Path, PathBuf};

use crate::backend::command::{CommandRunner, SystemCommandRunner};
use crate::backend::{Backend, BackendError, BackendOutput, SystemContext};
use crate::core::{
    Confidence, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Query, Relation,
    RelationKind,
};

#[derive(Debug, Clone)]
pub struct NixBackend<R = SystemCommandRunner> {
    runner: R,
}

impl NixBackend<SystemCommandRunner> {
    pub fn new() -> Self {
        Self {
            runner: SystemCommandRunner,
        }
    }
}

impl Default for NixBackend<SystemCommandRunner> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> NixBackend<R> {
    pub fn with_runner(runner: R) -> Self {
        Self { runner }
    }
}

impl<R> Backend for NixBackend<R>
where
    R: CommandRunner,
{
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
        query: &Query,
        graph: &mut EvidenceGraph,
    ) -> Result<BackendOutput, BackendError> {
        let mut output = BackendOutput::new();
        let mut enriched = false;

        if let Query::StorePath(path) = query {
            if let Some(match_id) = add_query_store_path(graph, path)? {
                output.matches.push(match_id);
                enriched = true;
            } else {
                output.incomplete.push(format!(
                    "nix could not identify a containing store path for {}",
                    path.display()
                ));
            }
        }

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

        for (entity_id, path) in path_entities {
            if let Some(store_path) = containing_store_path(&path) {
                add_store_path_relation(graph, entity_id, store_path)?;
                enriched = true;
            }
        }

        let store_paths: Vec<_> = graph
            .entities()
            .filter(|entity| entity.kind == EntityKind::StorePath)
            .map(|entity| (entity.id.clone(), entity.name.clone()))
            .collect();

        for (store_id, store_path) in store_paths {
            match self.query_roots(&store_path) {
                Ok(roots) => {
                    for root in roots {
                        add_reachable_from_relation(graph, store_id.clone(), root)?;
                        enriched = true;
                    }
                }
                Err(message) => output.incomplete.push(message),
            }
        }

        if enriched {
            Ok(output)
        } else {
            Ok(
                output
                    .with_incomplete("nix did not find any /nix/store paths in the current graph"),
            )
        }
    }
}

fn add_query_store_path(
    graph: &mut EvidenceGraph,
    query_path: &Path,
) -> Result<Option<EntityId>, BackendError> {
    let Some(store_path) = containing_store_path(query_path) else {
        return Ok(None);
    };

    if query_path == store_path {
        let store_path = display_path(&store_path)?;
        let store_id = EntityId::new(format!("store-path:{store_path}"));
        return Ok(Some(graph.add_entity(Entity::new(
            store_id,
            EntityKind::StorePath,
            store_path,
        ))));
    }

    let file_id = add_file_entity(graph, query_path, EntityKind::File)?;
    add_store_path_relation(graph, file_id.clone(), store_path)?;
    Ok(Some(file_id))
}

impl<R> NixBackend<R>
where
    R: CommandRunner,
{
    fn query_roots(&self, store_path: &str) -> Result<Vec<String>, String> {
        let output = self
            .runner
            .run("nix-store", &["--query", "--roots", store_path])
            .map_err(|error| format!("nix could not query roots for {store_path}: {error:?}"))?;

        if output.status != 0 {
            return Err(format!(
                "nix-store --query --roots failed for {store_path}: {}",
                output.stderr.trim()
            ));
        }

        Ok(parse_roots(&output.stdout))
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

fn add_file_entity(
    graph: &mut EvidenceGraph,
    path: &Path,
    kind: EntityKind,
) -> Result<EntityId, BackendError> {
    let path = display_path(path)?;
    let id = EntityId::new(format!("file:{path}"));
    Ok(graph.add_entity(Entity::new(id, kind, path)))
}

fn add_reachable_from_relation(
    graph: &mut EvidenceGraph,
    store_id: EntityId,
    root: String,
) -> Result<(), BackendError> {
    let root_id = EntityId::new(format!("file:{root}"));
    graph.add_entity(Entity::new(root_id.clone(), EntityKind::File, root));
    graph.add_relation(Relation::new(
        store_id,
        root_id,
        RelationKind::ReachableFrom,
        Confidence::Exact,
        vec![Evidence::new(
            "nix",
            "nix-store --query --roots",
            "Nix reports this path as a GC root keeping the store path reachable",
            Confidence::Exact,
        )],
    ));

    Ok(())
}

fn parse_roots(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.split_once(" -> ")
                .map(|(root, _)| root)
                .unwrap_or(line)
        })
        .map(ToOwned::to_owned)
        .collect()
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
    use crate::backend::command::{CommandError, CommandOutput, CommandRunner};
    use crate::backend::nix::{NixBackend, containing_store_path, parse_roots};
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

        let output = NixBackend::with_runner(FakeCommandRunner::stdout(""))
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

        NixBackend::with_runner(FakeCommandRunner::stdout(""))
            .investigate(&Query::Auto("sh".to_string()), &mut graph)
            .unwrap();

        let belongs_to = graph
            .relations()
            .filter(|relation| relation.kind == RelationKind::BelongsTo)
            .collect::<Vec<_>>();
        assert_eq!(belongs_to.len(), 1);
        assert_eq!(belongs_to[0].from, target_id);
    }

    #[test]
    fn parses_roots_output() {
        assert_eq!(
            parse_roots(
                "/run/current-system -> /nix/store/abc-system\n\n/nix/var/nix/profiles/system\n"
            ),
            vec![
                "/run/current-system".to_string(),
                "/nix/var/nix/profiles/system".to_string()
            ]
        );
    }

    #[test]
    fn adds_reachable_from_relations_for_roots() {
        let mut graph = EvidenceGraph::new();
        graph.add_entity(Entity::new(
            EntityId::new("file:/nix/store/abc123-bash/bin/bash"),
            EntityKind::Executable,
            "/nix/store/abc123-bash/bin/bash",
        ));

        NixBackend::with_runner(FakeCommandRunner::stdout("/run/current-system\n"))
            .investigate(&Query::Auto("bash".to_string()), &mut graph)
            .unwrap();

        let reachable_from = graph
            .relations()
            .filter(|relation| relation.kind == RelationKind::ReachableFrom)
            .collect::<Vec<_>>();
        assert_eq!(reachable_from.len(), 1);
        assert_eq!(
            graph.entity(&reachable_from[0].to).unwrap().name,
            "/run/current-system"
        );
    }

    #[test]
    fn direct_store_path_query_creates_store_path_match() {
        let mut graph = EvidenceGraph::new();
        let output = NixBackend::with_runner(FakeCommandRunner::stdout(""))
            .investigate(
                &Query::StorePath(Path::new("/nix/store/abc123-bash").to_path_buf()),
                &mut graph,
            )
            .unwrap();

        assert_eq!(output.matches.len(), 1);
        assert_eq!(
            graph.entity(&output.matches[0]).unwrap().kind,
            EntityKind::StorePath
        );
    }

    #[test]
    fn direct_path_inside_store_creates_file_match_and_belongs_to_relation() {
        let mut graph = EvidenceGraph::new();
        let output = NixBackend::with_runner(FakeCommandRunner::stdout(""))
            .investigate(
                &Query::StorePath(Path::new("/nix/store/abc123-bash/bin/bash").to_path_buf()),
                &mut graph,
            )
            .unwrap();

        assert_eq!(output.matches.len(), 1);
        assert_eq!(
            graph.entity(&output.matches[0]).unwrap().kind,
            EntityKind::File
        );
        assert!(
            graph
                .relations()
                .any(|relation| relation.kind == RelationKind::BelongsTo)
        );
    }

    #[derive(Debug, Clone)]
    struct FakeCommandRunner {
        output: Result<CommandOutput, CommandError>,
    }

    impl FakeCommandRunner {
        fn stdout(stdout: &str) -> Self {
            Self {
                output: Ok(CommandOutput {
                    status: 0,
                    stdout: stdout.to_string(),
                    stderr: String::new(),
                }),
            }
        }
    }

    impl CommandRunner for FakeCommandRunner {
        fn run(&self, _program: &str, _args: &[&str]) -> Result<CommandOutput, CommandError> {
            self.output.clone()
        }
    }
}
