use crate::backend::filesystem::FileSystemBackend;
use crate::backend::nix::NixBackend;
use crate::backend::procfs::ProcfsBackend;
use crate::backend::systemd::SystemdBackend;
use crate::backend::{Backend, BackendError, BackendState, BackendStatus, SystemContext};
use crate::core::{EntityId, EvidenceGraph, Query};

#[derive(Debug, Clone)]
pub struct Investigation {
    pub query: Query,
    pub answer: String,
    pub graph: EvidenceGraph,
    pub matches: Vec<EntityId>,
    pub incomplete: Vec<String>,
    pub backend_status: Vec<BackendStatus>,
}

impl Investigation {
    pub fn empty(query: Query) -> Self {
        Self {
            query,
            answer: "No explanation available yet.".to_string(),
            graph: EvidenceGraph::new(),
            matches: Vec::new(),
            incomplete: vec!["No backend has produced an explanation for this query.".to_string()],
            backend_status: vec![
                BackendStatus::new("filesystem", BackendState::NotImplemented),
                BackendStatus::new("nix", BackendState::NotImplemented),
                BackendStatus::new("procfs", BackendState::NotImplemented),
                BackendStatus::new("systemd", BackendState::NotImplemented),
            ],
        }
    }
}

#[derive(Debug, Default)]
pub struct Engine;

impl Engine {
    pub fn new() -> Self {
        Self
    }

    pub fn investigate(&self, query: Query) -> Investigation {
        let context = SystemContext::detect();
        let mut investigation = Investigation {
            query: query.clone(),
            answer: "No explanation available yet.".to_string(),
            graph: EvidenceGraph::new(),
            matches: Vec::new(),
            incomplete: Vec::new(),
            backend_status: Vec::new(),
        };

        let systemd = SystemdBackend::new();
        run_backend(&systemd, &context, &query, &mut investigation);

        let procfs = ProcfsBackend::new();
        run_backend(&procfs, &context, &query, &mut investigation);

        let filesystem = FileSystemBackend::from_context(&context);
        run_backend(&filesystem, &context, &query, &mut investigation);

        let nix = NixBackend::new();
        run_backend(&nix, &context, &query, &mut investigation);

        if investigation.matches.is_empty() && investigation.incomplete.is_empty() {
            investigation
                .incomplete
                .push("No backend has produced an explanation for this query.".to_string());
        }

        if !investigation.matches.is_empty() {
            investigation.answer = match &investigation.query {
                Query::Auto(_) => "Executable found in PATH.".to_string(),
                Query::File(_) => "Path found.".to_string(),
                Query::Process(_) => "Process found.".to_string(),
                Query::Service(_) => "Service found.".to_string(),
                Query::StorePath(_) => "Nix store path found.".to_string(),
                _ => "Explanation available.".to_string(),
            };
        }

        investigation
    }
}

fn run_backend<B: Backend>(
    backend: &B,
    context: &SystemContext,
    query: &Query,
    investigation: &mut Investigation,
) {
    if !backend.detect(context) {
        investigation.backend_status.push(BackendStatus::new(
            backend.name(),
            BackendState::Unavailable,
        ));
        return;
    }

    if !backend.supports(query) {
        investigation
            .backend_status
            .push(BackendStatus::new(backend.name(), BackendState::NotUsed));
        return;
    }

    match backend.investigate(query, &mut investigation.graph) {
        Ok(output) => {
            investigation.matches.extend(output.matches);
            investigation.incomplete.extend(output.incomplete);
            investigation
                .backend_status
                .push(BackendStatus::new(backend.name(), BackendState::Ok));
        }
        Err(BackendError::UnsupportedQuery) => {
            investigation
                .backend_status
                .push(BackendStatus::new(backend.name(), BackendState::NotUsed));
        }
        Err(BackendError::NotImplemented) => {
            investigation.backend_status.push(BackendStatus::new(
                backend.name(),
                BackendState::NotImplemented,
            ));
        }
        Err(BackendError::Failed(message)) => {
            investigation.backend_status.push(BackendStatus::new(
                backend.name(),
                BackendState::Error(message),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::backend::BackendState;
    use crate::core::Query;
    use crate::engine::Engine;

    #[test]
    fn unsupported_query_marks_filesystem_not_used() {
        let investigation = Engine::new().investigate(Query::Package("firefox".to_string()));

        let filesystem = investigation
            .backend_status
            .iter()
            .find(|status| status.backend == "filesystem")
            .unwrap();

        assert_eq!(filesystem.state, BackendState::NotUsed);
        assert!(investigation.matches.is_empty());
    }

    #[test]
    fn unsupported_query_still_runs_nix_as_graph_enricher_when_available() {
        let investigation = Engine::new().investigate(Query::Package("firefox".to_string()));

        let nix = investigation
            .backend_status
            .iter()
            .find(|status| status.backend == "nix")
            .unwrap();

        assert!(matches!(
            nix.state,
            BackendState::Ok | BackendState::Unavailable
        ));
    }

    #[test]
    fn unsupported_query_allows_procfs_enrichment() {
        let investigation = Engine::new().investigate(Query::Package("firefox".to_string()));

        let procfs = investigation
            .backend_status
            .iter()
            .find(|status| status.backend == "procfs")
            .unwrap();

        assert!(matches!(
            procfs.state,
            BackendState::Ok | BackendState::Unavailable
        ));
    }

    #[test]
    fn unsupported_query_marks_systemd_not_used() {
        let investigation = Engine::new().investigate(Query::Package("firefox".to_string()));

        let systemd = investigation
            .backend_status
            .iter()
            .find(|status| status.backend == "systemd")
            .unwrap();

        assert!(matches!(
            systemd.state,
            BackendState::NotUsed | BackendState::Unavailable
        ));
    }

    #[test]
    fn missing_auto_query_reports_filesystem_incomplete() {
        let investigation = Engine::new().investigate(Query::Auto(
            "syswhy-definitely-not-a-real-executable-name".to_string(),
        ));

        assert!(investigation.matches.is_empty());
        assert!(
            investigation
                .incomplete
                .iter()
                .any(|message| message.contains("could not find executable"))
        );
    }
}
