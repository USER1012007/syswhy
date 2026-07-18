use std::collections::BTreeMap;
use std::path::Path;

use crate::backend::command::{CommandRunner, SystemCommandRunner};
use crate::backend::{Backend, BackendError, BackendOutput, SystemContext};
use crate::core::{
    Confidence, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Query, Relation,
    RelationKind,
};

#[derive(Debug, Clone)]
pub struct SystemdBackend<R = SystemCommandRunner> {
    runner: R,
}

impl SystemdBackend<SystemCommandRunner> {
    pub fn new() -> Self {
        Self {
            runner: SystemCommandRunner,
        }
    }
}

impl Default for SystemdBackend<SystemCommandRunner> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> SystemdBackend<R> {
    pub fn with_runner(runner: R) -> Self {
        Self { runner }
    }
}

impl<R> Backend for SystemdBackend<R>
where
    R: CommandRunner,
{
    fn name(&self) -> &'static str {
        "systemd"
    }

    fn detect(&self, context: &SystemContext) -> bool {
        context.has_systemd
    }

    fn supports(&self, query: &Query) -> bool {
        matches!(query, Query::Service(_))
    }

    fn investigate(
        &self,
        query: &Query,
        graph: &mut EvidenceGraph,
    ) -> Result<BackendOutput, BackendError> {
        let Query::Service(service) = query else {
            return Err(BackendError::UnsupportedQuery);
        };

        let unit = normalize_unit_name(service);
        let properties = self.show_unit(&unit)?;
        if properties.is_empty() {
            return Ok(BackendOutput::new()
                .with_incomplete(format!("systemd returned no data for {unit}")));
        }

        let service_id = add_service_entity(graph, &unit, &properties);
        add_fragment_relation(graph, &service_id, &unit, &properties)?;
        add_main_pid_relation(graph, &service_id, &unit, &properties);

        Ok(BackendOutput::new().with_match(service_id))
    }
}

impl<R> SystemdBackend<R>
where
    R: CommandRunner,
{
    fn show_unit(&self, unit: &str) -> Result<BTreeMap<String, String>, BackendError> {
        let output = self
            .runner
            .run(
                "systemctl",
                &[
                    "show",
                    unit,
                    "--property=Id",
                    "--property=LoadState",
                    "--property=ActiveState",
                    "--property=SubState",
                    "--property=FragmentPath",
                    "--property=MainPID",
                ],
            )
            .map_err(|error| BackendError::Failed(format!("systemctl show failed: {error:?}")))?;

        if output.status != 0 {
            return Err(BackendError::Failed(format!(
                "systemctl show {unit} failed: {}",
                output.stderr.trim()
            )));
        }

        Ok(parse_show_output(&output.stdout))
    }
}

fn add_service_entity(
    graph: &mut EvidenceGraph,
    unit: &str,
    properties: &BTreeMap<String, String>,
) -> EntityId {
    let id = EntityId::new(format!("service:systemd:{unit}"));
    let name = properties
        .get("Id")
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| unit.to_string());
    let mut entity = Entity::new(id.clone(), EntityKind::Service, name);

    for key in ["LoadState", "ActiveState", "SubState", "MainPID"] {
        if let Some(value) = properties.get(key).filter(|value| !value.is_empty()) {
            entity.metadata.insert(to_metadata_key(key), value.clone());
        }
    }

    graph.add_entity(entity)
}

fn add_fragment_relation(
    graph: &mut EvidenceGraph,
    service_id: &EntityId,
    unit: &str,
    properties: &BTreeMap<String, String>,
) -> Result<(), BackendError> {
    let Some(fragment_path) = properties
        .get("FragmentPath")
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    let file_id = add_file_entity(graph, Path::new(fragment_path))?;
    graph.add_relation(Relation::new(
        service_id.clone(),
        file_id,
        RelationKind::ConfiguredBy,
        Confidence::Exact,
        vec![Evidence::new(
            "systemd",
            format!("systemctl show {unit} --property=FragmentPath"),
            "systemd reports this unit fragment path",
            Confidence::Exact,
        )],
    ));

    Ok(())
}

fn add_main_pid_relation(
    graph: &mut EvidenceGraph,
    service_id: &EntityId,
    unit: &str,
    properties: &BTreeMap<String, String>,
) {
    let Some(pid) = properties
        .get("MainPID")
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|pid| *pid != 0)
    else {
        return;
    };

    let process_id = EntityId::new(format!("process:{pid}"));
    graph.add_entity(Entity::new(
        process_id.clone(),
        EntityKind::Process,
        format!("PID {pid}"),
    ));
    graph.add_relation(Relation::new(
        service_id.clone(),
        process_id,
        RelationKind::Uses,
        Confidence::Exact,
        vec![Evidence::new(
            "systemd",
            format!("systemctl show {unit} --property=MainPID"),
            "systemd reports this unit main process",
            Confidence::Exact,
        )],
    ));
}

fn add_file_entity(graph: &mut EvidenceGraph, path: &Path) -> Result<EntityId, BackendError> {
    if path.as_os_str().is_empty() {
        return Err(BackendError::Failed(
            "empty systemd fragment path".to_string(),
        ));
    }

    let path = path.display().to_string();
    let id = EntityId::new(format!("file:{path}"));
    Ok(graph.add_entity(Entity::new(id, EntityKind::File, path)))
}

fn parse_show_output(stdout: &str) -> BTreeMap<String, String> {
    stdout
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn normalize_unit_name(service: &str) -> String {
    if service.contains('.') {
        service.to_string()
    } else {
        format!("{service}.service")
    }
}

fn to_metadata_key(key: &str) -> String {
    if key == "MainPID" {
        return "main_pid".to_string();
    }

    let mut result = String::new();
    for (index, character) in key.chars().enumerate() {
        if character.is_uppercase() {
            if index != 0 {
                result.push('_');
            }
            result.extend(character.to_lowercase());
        } else {
            result.push(character);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::backend::Backend;
    use crate::backend::command::{CommandError, CommandOutput, CommandRunner};
    use crate::backend::systemd::{SystemdBackend, normalize_unit_name, parse_show_output};
    use crate::core::{EntityKind, EvidenceGraph, Query, RelationKind};

    #[test]
    fn parses_systemctl_show_output() {
        let properties = parse_show_output(
            "Id=bluetooth.service\nActiveState=active\nFragmentPath=/nix/store/unit\nMainPID=42\n",
        );

        assert_eq!(properties.get("Id").unwrap(), "bluetooth.service");
        assert_eq!(properties.get("MainPID").unwrap(), "42");
    }

    #[test]
    fn normalizes_service_names() {
        assert_eq!(normalize_unit_name("bluetooth"), "bluetooth.service");
        assert_eq!(normalize_unit_name("ssh.socket"), "ssh.socket");
    }

    #[test]
    fn service_query_creates_service_relations() {
        let stdout = "\
Id=bluetooth.service
LoadState=loaded
ActiveState=active
SubState=running
FragmentPath=/nix/store/abc-unit/bluetooth.service
MainPID=42
";
        let backend = SystemdBackend::with_runner(FakeCommandRunner::stdout(stdout));
        let mut graph = EvidenceGraph::new();
        let output = backend
            .investigate(&Query::Service("bluetooth".to_string()), &mut graph)
            .unwrap();

        assert_eq!(output.matches.len(), 1);
        assert_eq!(
            graph.entity(&output.matches[0]).unwrap().kind,
            EntityKind::Service
        );
        assert!(
            graph
                .relations()
                .any(|relation| relation.kind == RelationKind::ConfiguredBy)
        );
        assert!(
            graph
                .relations()
                .any(|relation| relation.kind == RelationKind::Uses)
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
