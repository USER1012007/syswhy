use std::collections::BTreeMap;
use std::fmt;

pub type Metadata = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(String);

impl EntityId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntityKind {
    File,
    Executable,
    Package,
    Process,
    Service,
    Port,
    Socket,
    Mount,
    Configuration,
    StorePath,
    Derivation,
    Generation,
    KernelModule,
    Device,
}

impl EntityKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Executable => "executable",
            Self::Package => "package",
            Self::Process => "process",
            Self::Service => "service",
            Self::Port => "port",
            Self::Socket => "socket",
            Self::Mount => "mount",
            Self::Configuration => "configuration",
            Self::StorePath => "store_path",
            Self::Derivation => "derivation",
            Self::Generation => "generation",
            Self::KernelModule => "kernel_module",
            Self::Device => "device",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub name: String,
    pub metadata: Metadata,
}

impl Entity {
    pub fn new(id: EntityId, kind: EntityKind, name: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            name: name.into(),
            metadata: Metadata::new(),
        }
    }
}
