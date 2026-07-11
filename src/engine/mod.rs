use crate::backend::{BackendState, BackendStatus};
use crate::core::{EntityId, EvidenceGraph, Query};

#[derive(Debug, Clone)]
pub struct Investigation {
    pub query: Query,
    pub graph: EvidenceGraph,
    pub matches: Vec<EntityId>,
    pub incomplete: Vec<String>,
    pub backend_status: Vec<BackendStatus>,
}

impl Investigation {
    pub fn empty(query: Query) -> Self {
        Self {
            query,
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
        Investigation::empty(query)
    }
}
