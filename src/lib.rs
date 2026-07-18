//! Library API for `syswhy`.
//!
//! The binary is intentionally thin: it parses CLI arguments, runs the engine,
//! and renders the resulting investigation. Other frontends, such as a separate
//! TUI crate, should use this library API instead of shelling out to `syswhy`.

pub mod backend;
pub mod cli;
pub mod core;
pub mod engine;

pub use cli::json::render as render_json;
pub use cli::plain::{PlainRenderMode, render as render_plain};
pub use core::{
    Confidence, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Metadata, Protocol, Query,
    Relation, RelationKind, SyswhyError,
};
pub use engine::{Engine, Investigation};

/// Common imports for frontend crates.
///
/// This is intended for consumers such as `syswhy-tui`, examples, and small
/// integrations that need to run an investigation and inspect the evidence graph.
pub mod prelude {
    pub use crate::{
        Confidence, Engine, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Investigation,
        Metadata, PlainRenderMode, Protocol, Query, Relation, RelationKind, SyswhyError,
        render_json, render_plain,
    };
}
