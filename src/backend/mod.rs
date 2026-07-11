use crate::core::{EvidenceGraph, Query};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendState {
    Ok,
    NotUsed,
    NotImplemented,
    Unavailable,
    Error(String),
}

impl BackendState {
    pub fn as_display(&self) -> &str {
        match self {
            Self::Ok => "ok",
            Self::NotUsed => "not used",
            Self::NotImplemented => "not implemented",
            Self::Unavailable => "unavailable",
            Self::Error(message) => message.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendStatus {
    pub backend: String,
    pub state: BackendState,
}

impl BackendStatus {
    pub fn new(backend: impl Into<String>, state: BackendState) -> Self {
        Self {
            backend: backend.into(),
            state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemContext {
    pub has_systemd: bool,
    pub has_nix_store: bool,
    pub path_env: Option<String>,
}

impl SystemContext {
    pub fn detect() -> Self {
        Self {
            has_systemd: std::path::Path::new("/run/systemd/system").exists(),
            has_nix_store: std::path::Path::new("/nix/store").exists(),
            path_env: std::env::var("PATH").ok(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    UnsupportedQuery,
    NotImplemented,
    Failed(String),
}

pub trait Backend {
    fn name(&self) -> &'static str;

    fn detect(&self, context: &SystemContext) -> bool;

    fn supports(&self, query: &Query) -> bool;

    fn investigate(&self, query: &Query, graph: &mut EvidenceGraph) -> Result<(), BackendError>;
}
