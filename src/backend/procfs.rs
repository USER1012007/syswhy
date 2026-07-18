use std::fs;
use std::path::{Path, PathBuf};

use crate::backend::{Backend, BackendError, BackendOutput, SystemContext};
use crate::core::{
    Confidence, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Query, Relation,
    RelationKind,
};

#[derive(Debug, Clone)]
pub struct ProcfsBackend {
    proc_root: PathBuf,
}

impl ProcfsBackend {
    pub fn new() -> Self {
        Self {
            proc_root: PathBuf::from("/proc"),
        }
    }

    pub fn with_proc_root(proc_root: impl Into<PathBuf>) -> Self {
        Self {
            proc_root: proc_root.into(),
        }
    }

    fn investigate_pid(
        &self,
        pid: u32,
        graph: &mut EvidenceGraph,
        include_match: bool,
    ) -> Result<BackendOutput, BackendError> {
        let process_dir = self.proc_root.join(pid.to_string());
        if !process_dir.exists() {
            return Ok(
                BackendOutput::new().with_incomplete(format!("procfs could not find PID {pid}"))
            );
        }

        let process_id = add_process_entity(graph, pid, &process_dir);
        let mut output = BackendOutput::new();
        if include_match {
            output.matches.push(process_id.clone());
        }

        if let Some(parent_pid) = read_parent_pid(&process_dir) {
            let parent_dir = self.proc_root.join(parent_pid.to_string());
            let parent_id = add_process_entity(graph, parent_pid, &parent_dir);
            graph.add_relation(Relation::new(
                process_id.clone(),
                parent_id,
                RelationKind::StartedBy,
                Confidence::Exact,
                vec![Evidence::new(
                    "procfs",
                    format!("/proc/{pid}/status"),
                    "Process status reports this parent PID",
                    Confidence::Exact,
                )],
            ));
        }

        match fs::read_link(process_dir.join("exe")) {
            Ok(exe_path) => {
                let exe_id = add_file_entity(graph, &exe_path, EntityKind::Executable)?;
                graph.add_relation(Relation::new(
                    process_id,
                    exe_id,
                    RelationKind::Uses,
                    Confidence::Exact,
                    vec![Evidence::new(
                        "procfs",
                        format!("/proc/{pid}/exe"),
                        "Process executable symlink points to this path",
                        Confidence::Exact,
                    )],
                ));
            }
            Err(error) => output.incomplete.push(format!(
                "procfs could not read executable for PID {pid}: {error}"
            )),
        }

        Ok(output)
    }
}

impl Default for ProcfsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for ProcfsBackend {
    fn name(&self) -> &'static str {
        "procfs"
    }

    fn detect(&self, _context: &SystemContext) -> bool {
        self.proc_root.exists()
    }

    fn supports(&self, _query: &Query) -> bool {
        true
    }

    fn investigate(
        &self,
        query: &Query,
        graph: &mut EvidenceGraph,
    ) -> Result<BackendOutput, BackendError> {
        match query {
            Query::Process(pid) => self.investigate_pid(*pid, graph, true),
            _ => self.enrich_existing_processes(graph),
        }
    }
}

impl ProcfsBackend {
    fn enrich_existing_processes(
        &self,
        graph: &mut EvidenceGraph,
    ) -> Result<BackendOutput, BackendError> {
        let pids = graph
            .entities()
            .filter(|entity| entity.kind == EntityKind::Process)
            .filter_map(|entity| parse_process_id(&entity.id))
            .collect::<Vec<_>>();

        let mut output = BackendOutput::new();
        for pid in pids {
            let pid_output = self.investigate_pid(pid, graph, false)?;
            output.incomplete.extend(pid_output.incomplete);
        }

        Ok(output)
    }
}

fn add_process_entity(graph: &mut EvidenceGraph, pid: u32, process_dir: &Path) -> EntityId {
    let id = EntityId::new(format!("process:{pid}"));
    let mut entity = Entity::new(id.clone(), EntityKind::Process, format!("PID {pid}"));

    if let Ok(comm) = fs::read_to_string(process_dir.join("comm")) {
        let comm = comm.trim();
        if !comm.is_empty() {
            entity.metadata.insert("comm".to_string(), comm.to_string());
            entity.name = format!("{comm} ({pid})");
        }
    }

    if let Ok(cmdline) = fs::read(process_dir.join("cmdline")) {
        let cmdline = format_cmdline(&cmdline);
        if !cmdline.is_empty() {
            entity.metadata.insert("cmdline".to_string(), cmdline);
        }
    }

    if let Ok(cwd) = fs::read_link(process_dir.join("cwd")) {
        entity
            .metadata
            .insert("cwd".to_string(), cwd.display().to_string());
    }

    graph.add_entity(entity)
}

fn read_parent_pid(process_dir: &Path) -> Option<u32> {
    let status = fs::read_to_string(process_dir.join("status")).ok()?;
    status.lines().find_map(|line| {
        line.strip_prefix("PPid:")
            .and_then(|value| value.trim().parse::<u32>().ok())
            .filter(|pid| *pid != 0)
    })
}

fn parse_process_id(entity_id: &EntityId) -> Option<u32> {
    entity_id.as_str().strip_prefix("process:")?.parse().ok()
}

fn add_file_entity(
    graph: &mut EvidenceGraph,
    path: &Path,
    kind: EntityKind,
) -> Result<EntityId, BackendError> {
    if path.as_os_str().is_empty() {
        return Err(BackendError::Failed(
            "empty procfs executable path".to_string(),
        ));
    }

    let path = path.display().to_string();
    let id = EntityId::new(format!("file:{path}"));
    Ok(graph.add_entity(Entity::new(id, kind, path)))
}

fn format_cmdline(bytes: &[u8]) -> String {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::backend::Backend;
    use crate::backend::procfs::{ProcfsBackend, format_cmdline, read_parent_pid};
    use crate::core::{Entity, EntityId, EntityKind, EvidenceGraph, Query, RelationKind};

    #[test]
    fn formats_nul_separated_cmdline() {
        assert_eq!(format_cmdline(b"bash\0-l\0"), "bash -l");
    }

    #[test]
    fn pid_query_creates_process_and_executable_relation() {
        let fixture = TempFixture::new();
        let proc_dir = fixture.path.join("42");
        fs::create_dir(&proc_dir).unwrap();
        fs::write(proc_dir.join("comm"), "demo\n").unwrap();
        fs::write(proc_dir.join("cmdline"), b"demo\0--flag\0").unwrap();
        fs::write(proc_dir.join("status"), "Name:\tdemo\nPPid:\t7\n").unwrap();
        let parent_dir = fixture.path.join("7");
        fs::create_dir(&parent_dir).unwrap();
        fs::write(parent_dir.join("comm"), "parent\n").unwrap();
        let exe = fixture.path.join("bin-demo");
        fs::write(&exe, "").unwrap();
        std::os::unix::fs::symlink(&exe, proc_dir.join("exe")).unwrap();

        let backend = ProcfsBackend::with_proc_root(&fixture.path);
        let mut graph = EvidenceGraph::new();
        let output = backend
            .investigate(&Query::Process(42), &mut graph)
            .unwrap();

        assert_eq!(output.matches.len(), 1);
        assert_eq!(
            graph.entity(&output.matches[0]).unwrap().kind,
            EntityKind::Process
        );
        assert!(
            graph
                .relations()
                .any(|relation| relation.kind == RelationKind::Uses)
        );
        assert!(
            graph
                .relations()
                .any(|relation| relation.kind == RelationKind::StartedBy)
        );
    }

    #[test]
    fn reads_parent_pid_from_status() {
        let fixture = TempFixture::new();
        fs::write(fixture.path.join("status"), "Name:\tdemo\nPPid:\t123\n").unwrap();

        assert_eq!(read_parent_pid(&fixture.path), Some(123));
    }

    #[test]
    fn enriches_existing_process_without_adding_match() {
        let fixture = TempFixture::new();
        let proc_dir = fixture.path.join("42");
        fs::create_dir(&proc_dir).unwrap();
        fs::write(proc_dir.join("comm"), "demo\n").unwrap();
        let exe = fixture.path.join("bin-demo");
        fs::write(&exe, "").unwrap();
        std::os::unix::fs::symlink(&exe, proc_dir.join("exe")).unwrap();

        let backend = ProcfsBackend::with_proc_root(&fixture.path);
        let mut graph = EvidenceGraph::new();
        graph.add_entity(Entity::new(
            EntityId::new("process:42"),
            EntityKind::Process,
            "PID 42",
        ));

        let output = backend
            .investigate(&Query::Service("demo".to_string()), &mut graph)
            .unwrap();

        assert!(output.matches.is_empty());
        assert!(
            graph
                .relations()
                .any(|relation| relation.kind == RelationKind::Uses)
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
                "syswhy-procfs-test-{}-{unique}",
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
