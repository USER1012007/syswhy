use crate::core::Relation;
use crate::engine::Investigation;

pub fn render(investigation: &Investigation, include_evidence: bool) -> String {
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

    if include_evidence {
        let evidence = collect_evidence(investigation);
        if !evidence.is_empty() {
            push_line(&mut output, "");
            push_line(&mut output, "Evidence:");
            for (id, evidence) in evidence {
                push_line(
                    &mut output,
                    &format!(
                        "  [{id}] {} | {} | {}",
                        evidence.backend, evidence.source, evidence.confidence
                    ),
                );
            }
        }
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

    output
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

fn collect_evidence(investigation: &Investigation) -> Vec<(String, &crate::core::Evidence)> {
    let mut evidence = Vec::new();
    let mut index = 1;

    for relation in investigation.graph.relations() {
        for item in &relation.evidence {
            evidence.push((format!("e{index}"), item));
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
    use crate::core::Query;
    use crate::engine::Investigation;

    #[test]
    fn renders_empty_phase_zero_investigation() {
        let investigation = Investigation::empty(Query::Auto("firefox".to_string()));
        let output = plain::render(&investigation, false);

        assert!(output.contains("Query: firefox"));
        assert!(output.contains("Interpreted as: auto search"));
        assert!(output.contains("No explanation available yet."));
        assert!(output.contains("No backend has produced an explanation"));
        assert!(output.contains("filesystem not implemented"));
    }
}
