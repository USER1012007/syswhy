use crate::core::{Confidence, EntityId, Evidence};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RelationKind {
    Owns,
    Requires,
    References,
    RefersTo,
    StartedBy,
    EnabledBy,
    ConfiguredBy,
    DeclaredAt,
    InstalledBecauseOf,
    Exposes,
    ResolvesTo,
    GeneratedFrom,
    KeptAliveBy,
    LoadedBy,
    BelongsTo,
    ReachableFrom,
    Uses,
}

impl RelationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owns => "owns",
            Self::Requires => "requires",
            Self::References => "references",
            Self::RefersTo => "refers_to",
            Self::StartedBy => "started_by",
            Self::EnabledBy => "enabled_by",
            Self::ConfiguredBy => "configured_by",
            Self::DeclaredAt => "declared_at",
            Self::InstalledBecauseOf => "installed_because_of",
            Self::Exposes => "exposes",
            Self::ResolvesTo => "resolves_to",
            Self::GeneratedFrom => "generated_from",
            Self::KeptAliveBy => "kept_alive_by",
            Self::LoadedBy => "loaded_by",
            Self::BelongsTo => "belongs_to",
            Self::ReachableFrom => "reachable_from",
            Self::Uses => "uses",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::ResolvesTo => "resolves to",
            Self::BelongsTo => "belongs to",
            Self::ReachableFrom => "kept because of",
            Self::StartedBy => "started by",
            Self::ConfiguredBy => "configured by",
            Self::DeclaredAt => "declared in",
            Self::Owns => "owns",
            Self::Requires => "requires",
            Self::References => "references",
            Self::GeneratedFrom => "generated from",
            Self::KeptAliveBy => "kept alive by",
            Self::Uses => "uses",
            Self::Exposes => "exposes",
            Self::RefersTo => "refers to",
            Self::EnabledBy => "enabled by",
            Self::InstalledBecauseOf => "installed because of",
            Self::LoadedBy => "loaded by",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Relation {
    pub from: EntityId,
    pub to: EntityId,
    pub kind: RelationKind,
    pub evidence: Vec<Evidence>,
    pub confidence: Confidence,
}

impl Relation {
    pub fn new(
        from: EntityId,
        to: EntityId,
        kind: RelationKind,
        confidence: Confidence,
        evidence: Vec<Evidence>,
    ) -> Self {
        Self {
            from,
            to,
            kind,
            evidence,
            confidence,
        }
    }
}
