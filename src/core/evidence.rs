use crate::core::{Confidence, Metadata};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub backend: String,
    pub source: String,
    pub description: String,
    pub confidence: Confidence,
    pub metadata: Metadata,
}

impl Evidence {
    pub fn new(
        backend: impl Into<String>,
        source: impl Into<String>,
        description: impl Into<String>,
        confidence: Confidence,
    ) -> Self {
        Self {
            backend: backend.into(),
            source: source.into(),
            description: description.into(),
            confidence,
            metadata: Metadata::new(),
        }
    }
}
