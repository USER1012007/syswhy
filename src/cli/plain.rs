use crate::core::{Evidence, Relation};
use crate::engine::Investigation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlainRenderMode {
    Compact,
    Evidence,
    Full,
    Debug,
}

pub fn render(investigation: &Investigation, mode: PlainRenderMode) -> String {
    let mut output = String::new();

    push_line(
        &mut output,
        &format!("Query: {}", investigation.query.value()),
    );
    push_line(
        &mut output,
        &format!("Interpreted as: {}", investigation.query.interpreted_as()),
    );
    push_line(&mut output, "");

    push_line(&mut output, "Answer:");
    push_line(&mut output, &format!("  {}", investigation.answer));

    if !investigation.matches.is_empty() {
        push_line(&mut output, "");
        push_line(&mut output, "Matches:");
        for (index, entity_id) in investigation.matches.iter().enumerate() {
            let marker = if index == 0 { ">" } else { " " };
            if let Some(entity) = investigation.graph.entity(entity_id) {
                push_line(
                    &mut output,
                    &format!("  {marker} {} {}", entity.kind.as_str(), entity.name),
                );
            } else {
                push_line(&mut output, &format!("  {marker} {entity_id}"));
            }
        }
    }

    if !investigation.graph.is_empty() {
        push_line(&mut output, "");
        push_line(&mut output, "Main chain:");
        for entity in investigation.graph.entities() {
            if investigation.graph.incoming(&entity.id).next().is_none() {
                render_entity(&mut output, investigation, &entity.id, 0);
            }
        }
    }

    if mode.includes_evidence() {
        render_evidence(&mut output, investigation);
    }

    if mode.includes_full_graph() {
        render_graph_details(&mut output, investigation);
    }

    if !investigation.incomplete.is_empty() {
        push_line(&mut output, "");
        push_line(&mut output, "Incomplete:");
        for message in &investigation.incomplete {
            push_line(&mut output, &format!("  {message}"));
        }
    }

    if !investigation.backend_status.is_empty() {
        push_line(&mut output, "");
        push_line(&mut output, "Backend status:");
        let width = investigation
            .backend_status
            .iter()
            .map(|status| status.backend.len())
            .max()
            .unwrap_or(0);
        for status in &investigation.backend_status {
            push_line(
                &mut output,
                &format!(
                    "  {backend:<width$} {state}",
                    backend = status.backend,
                    width = width,
                    state = status.state.as_display()
                ),
            );
        }
    }

    if mode.includes_debug() {
        render_debug(&mut output, investigation);
    }

    output
}

impl PlainRenderMode {
    fn includes_evidence(self) -> bool {
        matches!(self, Self::Evidence | Self::Full | Self::Debug)
    }

    fn includes_full_graph(self) -> bool {
        matches!(self, Self::Full | Self::Debug)
    }

    fn includes_debug(self) -> bool {
        matches!(self, Self::Debug)
    }
}

fn render_evidence(output: &mut String, investigation: &Investigation) {
    let evidence = collect_evidence(investigation);
    if evidence.is_empty() {
        return;
    }

    push_line(output, "");
    push_line(output, "Evidence:");
    for entry in evidence {
        push_line(output, &format!("  [{}]", entry.id));
        push_line(
            output,
            &format!(
                "    Relation: {} --{}--> {}",
                entry.from,
                entry.relation.kind.label(),
                entry.to
            ),
        );
        push_line(output, &format!("    Backend: {}", entry.evidence.backend));
        push_line(output, &format!("    Source: {}", entry.evidence.source));
        push_line(
            output,
            &format!("    Confidence: {}", entry.evidence.confidence),
        );
        if !entry.evidence.description.is_empty() {
            push_line(
                output,
                &format!("    Description: {}", entry.evidence.description),
            );
        }
    }
}

fn render_graph_details(output: &mut String, investigation: &Investigation) {
    if investigation.graph.is_empty() {
        return;
    }

    push_line(output, "");
    push_line(output, "Graph details:");
    push_line(output, "  Entities:");
    for entity in investigation.graph.entities() {
        push_line(
            output,
            &format!(
                "    {} | {} | {}",
                entity.id.as_str(),
                entity.kind.as_str(),
                entity.name
            ),
        );
    }

    push_line(output, "  Relations:");
    for relation in investigation.graph.relations() {
        push_line(
            output,
            &format!(
                "    {} --{}--> {} | {}",
                relation.from,
                relation.kind.as_str(),
                relation.to,
                relation.confidence
            ),
        );
    }
}

fn render_debug(output: &mut String, investigation: &Investigation) {
    push_line(output, "");
    push_line(output, "Debug:");
    push_line(
        output,
        &format!("  Query kind: {}", investigation.query.kind()),
    );
    push_line(
        output,
        &format!("  Entity count: {}", investigation.graph.entity_count()),
    );
    push_line(
        output,
        &format!("  Relation count: {}", investigation.graph.relation_count()),
    );
    push_line(
        output,
        &format!("  Match count: {}", investigation.matches.len()),
    );
    push_line(
        output,
        &format!("  Incomplete count: {}", investigation.incomplete.len()),
    );
}

fn render_entity(
    output: &mut String,
    investigation: &Investigation,
    entity_id: &crate::core::EntityId,
    depth: usize,
) {
    let Some(entity) = investigation.graph.entity(entity_id) else {
        return;
    };

    if depth == 0 {
        push_line(output, &entity.name);
    } else {
        push_line(output, &format!("{}└── {}", indent(depth), entity.name));
    }

    for relation in investigation.graph.outgoing(entity_id) {
        let marker = evidence_marker(investigation, relation);
        push_line(
            output,
            &format!(
                "{}└── {}{}",
                indent(depth + 1),
                relation.kind.label(),
                marker
            ),
        );
        render_entity(output, investigation, &relation.to, depth + 2);
    }
}

struct EvidenceEntry<'a> {
    id: String,
    relation: &'a Relation,
    evidence: &'a Evidence,
    from: String,
    to: String,
}

fn collect_evidence(investigation: &Investigation) -> Vec<EvidenceEntry<'_>> {
    let mut evidence = Vec::new();
    let mut index = 1;

    for relation in investigation.graph.relations() {
        let from = investigation
            .graph
            .entity(&relation.from)
            .map(|entity| entity.name.clone())
            .unwrap_or_else(|| relation.from.to_string());
        let to = investigation
            .graph
            .entity(&relation.to)
            .map(|entity| entity.name.clone())
            .unwrap_or_else(|| relation.to.to_string());

        for item in &relation.evidence {
            evidence.push(EvidenceEntry {
                id: format!("e{index}"),
                relation,
                evidence: item,
                from: from.clone(),
                to: to.clone(),
            });
            index += 1;
        }
    }

    evidence
}

fn evidence_marker(investigation: &Investigation, relation: &Relation) -> String {
    if relation.evidence.is_empty() {
        return String::new();
    }

    let mut index = 1;
    for existing in investigation.graph.relations() {
        for evidence in &existing.evidence {
            if existing.from == relation.from
                && existing.to == relation.to
                && existing.kind == relation.kind
            {
                return format!(" [e{index} {}]", evidence.confidence);
            }
            index += 1;
        }
    }

    String::new()
}

fn indent(depth: usize) -> String {
    "    ".repeat(depth)
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use crate::cli::plain;
    use crate::core::{
        Confidence, Entity, EntityId, EntityKind, Evidence, EvidenceGraph, Query, Relation,
        RelationKind,
    };
    use crate::engine::Investigation;

    #[test]
    fn renders_empty_phase_zero_investigation() {
        let investigation = Investigation::empty(Query::Auto("firefox".to_string()));
        let output = plain::render(&investigation, plain::PlainRenderMode::Compact);

        assert!(output.contains("Query: firefox"));
        assert!(output.contains("Interpreted as: auto search"));
        assert!(output.contains("No explanation available yet."));
        assert!(output.contains("No backend has produced an explanation"));
        assert!(output.contains("filesystem not implemented"));
    }

    #[test]
    fn renders_detailed_evidence_section() {
        let mut graph = EvidenceGraph::new();
        let from = graph.add_entity(Entity::new(
            EntityId::new("file:/tmp/link"),
            EntityKind::Executable,
            "/tmp/link",
        ));
        let to = graph.add_entity(Entity::new(
            EntityId::new("file:/tmp/target"),
            EntityKind::File,
            "/tmp/target",
        ));
        graph.add_relation(Relation::new(
            from.clone(),
            to,
            RelationKind::ResolvesTo,
            Confidence::Exact,
            vec![Evidence::new(
                "filesystem",
                "std::fs::canonicalize",
                "Path resolves to this canonical target",
                Confidence::Exact,
            )],
        ));

        let investigation = Investigation {
            query: Query::Auto("link".to_string()),
            answer: "Found an executable in PATH.".to_string(),
            graph,
            matches: vec![from],
            incomplete: Vec::new(),
            backend_status: Vec::new(),
        };

        let output = plain::render(&investigation, plain::PlainRenderMode::Evidence);

        assert!(output.contains("[e1 exact]"));
        assert!(output.contains("Evidence:"));
        assert!(output.contains("Relation: /tmp/link --resolves to--> /tmp/target"));
        assert!(output.contains("Backend: filesystem"));
        assert!(output.contains("Description: Path resolves to this canonical target"));
    }

    #[test]
    fn full_mode_renders_graph_details() {
        let investigation = investigation_with_one_relation();
        let output = plain::render(&investigation, plain::PlainRenderMode::Full);

        assert!(output.contains("Evidence:"));
        assert!(output.contains("Graph details:"));
        assert!(output.contains("Entities:"));
        assert!(output.contains("Relations:"));
    }

    #[test]
    fn debug_mode_renders_debug_diagnostics() {
        let investigation = investigation_with_one_relation();
        let output = plain::render(&investigation, plain::PlainRenderMode::Debug);

        assert!(output.contains("Graph details:"));
        assert!(output.contains("Debug:"));
        assert!(output.contains("Entity count: 2"));
        assert!(output.contains("Relation count: 1"));
    }

    fn investigation_with_one_relation() -> Investigation {
        let mut graph = EvidenceGraph::new();
        let from = graph.add_entity(Entity::new(
            EntityId::new("file:/tmp/link"),
            EntityKind::Executable,
            "/tmp/link",
        ));
        let to = graph.add_entity(Entity::new(
            EntityId::new("file:/tmp/target"),
            EntityKind::File,
            "/tmp/target",
        ));
        graph.add_relation(Relation::new(
            from.clone(),
            to,
            RelationKind::ResolvesTo,
            Confidence::Exact,
            vec![Evidence::new(
                "filesystem",
                "std::fs::canonicalize",
                "Path resolves to this canonical target",
                Confidence::Exact,
            )],
        ));

        Investigation {
            query: Query::Auto("link".to_string()),
            answer: "Found an executable in PATH.".to_string(),
            graph,
            matches: vec![from],
            incomplete: Vec::new(),
            backend_status: Vec::new(),
        }
    }
}
