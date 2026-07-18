use crate::core::{Confidence, EntityId, Evidence, Relation, RelationKind};
use crate::engine::Investigation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlainRenderMode {
    Compact,
    Evidence,
    Full,
    Debug,
}

pub fn render(investigation: &Investigation, mode: PlainRenderMode) -> String {
    render_with_color(investigation, mode, false)
}

pub fn render_with_color(
    investigation: &Investigation,
    mode: PlainRenderMode,
    color: bool,
) -> String {
    let mut output = String::new();

    push_section(&mut output, "Query", color);
    push_line(&mut output, &format!("  {}", investigation.query.value()));
    push_line(
        &mut output,
        &format!("  interpreted as {}", investigation.query.interpreted_as()),
    );

    push_section(&mut output, "Answer", color);
    push_line(&mut output, &format!("  {}", investigation.answer));

    if !investigation.matches.is_empty() {
        push_section(&mut output, "Matches", color);
        for (index, entity_id) in investigation.matches.iter().enumerate() {
            let marker = if index == 0 {
                styled(">", "36", color)
            } else {
                " ".to_string()
            };
            if let Some(entity) = investigation.graph.entity(entity_id) {
                push_line(
                    &mut output,
                    &format!("  {marker} {:<11} {}", entity.kind.as_str(), entity.name),
                );
            } else {
                push_line(&mut output, &format!("  {marker} {entity_id}"));
            }
        }
    }

    if !investigation.graph.is_empty() {
        push_section(&mut output, "Why", color);
        for entity_id in why_roots(investigation) {
            render_entity(&mut output, investigation, &entity_id, mode, color);
        }
    }

    if mode.includes_evidence() {
        render_evidence(&mut output, investigation, color);
    }

    if mode.includes_full_graph() {
        render_graph_details(&mut output, investigation, color);
    }

    if !investigation.incomplete.is_empty() {
        push_section(&mut output, "Incomplete", color);
        for message in &investigation.incomplete {
            push_line(&mut output, &format!("  {message}"));
        }
    }

    if !investigation.backend_status.is_empty() {
        push_section(&mut output, "Backends", color);
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
        render_debug(&mut output, investigation, color);
    }

    output
}

impl PlainRenderMode {
    fn simplifies_graph(self) -> bool {
        matches!(self, Self::Compact | Self::Evidence)
    }

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

fn render_evidence(output: &mut String, investigation: &Investigation, color: bool) {
    let evidence = collect_evidence(investigation);
    if evidence.is_empty() {
        return;
    }

    push_section(output, "Evidence", color);
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

fn render_graph_details(output: &mut String, investigation: &Investigation, color: bool) {
    if investigation.graph.is_empty() {
        return;
    }

    push_section(output, "Graph", color);
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
        for (key, value) in &entity.metadata {
            push_line(output, &format!("      {key}: {value}"));
        }
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

fn render_debug(output: &mut String, investigation: &Investigation, color: bool) {
    push_section(output, "Debug", color);
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
    entity_id: &EntityId,
    mode: PlainRenderMode,
    color: bool,
) {
    let Some(entity) = investigation.graph.entity(entity_id) else {
        return;
    };

    push_line(output, &entity.name);
    render_children(output, investigation, entity_id, "", mode, color);
}

fn render_children(
    output: &mut String,
    investigation: &Investigation,
    entity_id: &EntityId,
    prefix: &str,
    mode: PlainRenderMode,
    color: bool,
) {
    let items = render_items(investigation, entity_id, mode);

    for (index, item) in items.iter().enumerate() {
        let is_last = index + 1 == items.len();
        let branch = if is_last { "└──" } else { "├──" };
        let child_prefix = if is_last { "    " } else { "│   " };
        let marker = evidence_marker(&item.evidence);
        let marker = styled(&marker, "90", color);
        let target = styled(&item.target_label, "97", color);
        push_line(
            output,
            &format!(
                "{prefix}{} {} {}{}",
                styled(branch, "90", color),
                item.kind.label(),
                target,
                marker,
            ),
        );
        render_children(
            output,
            investigation,
            &item.to,
            &format!("{prefix}{child_prefix}"),
            mode,
            color,
        );
    }
}

#[derive(Debug, Clone)]
struct RenderItem {
    to: EntityId,
    kind: RelationKind,
    target_label: String,
    evidence: Vec<EvidenceRef>,
    target_count: usize,
}

#[derive(Debug, Clone)]
struct EvidenceRef {
    id: String,
    confidence: Confidence,
}

fn why_roots(investigation: &Investigation) -> Vec<EntityId> {
    if !investigation.matches.is_empty() {
        return dedup_entity_ids(investigation.matches.iter().cloned());
    }

    dedup_entity_ids(
        investigation
            .graph
            .entities()
            .filter(|entity| investigation.graph.incoming(&entity.id).next().is_none())
            .map(|entity| entity.id.clone()),
    )
}

fn dedup_entity_ids(ids: impl IntoIterator<Item = EntityId>) -> Vec<EntityId> {
    let mut result = Vec::new();
    for id in ids {
        if !result.contains(&id) {
            result.push(id);
        }
    }
    result
}

fn render_items(
    investigation: &Investigation,
    entity_id: &EntityId,
    mode: PlainRenderMode,
) -> Vec<RenderItem> {
    let mut items = Vec::new();

    for relation in investigation.graph.outgoing(entity_id) {
        let compact_systemd_unit =
            mode.simplifies_graph() && is_low_signal_systemd_unit(investigation, relation);
        let target_label = if compact_systemd_unit {
            "systemd base units".to_string()
        } else {
            relation_target_label(investigation, relation)
        };
        let can_group =
            compact_systemd_unit || investigation.graph.outgoing(&relation.to).next().is_none();

        let existing_index = can_group
            .then(|| {
                items.iter().position(|item: &RenderItem| {
                    item.kind == relation.kind && item.target_label == target_label
                })
            })
            .flatten();

        if let Some(existing_index) = existing_index {
            items[existing_index]
                .evidence
                .extend(evidence_refs(investigation, relation));
            items[existing_index].target_count += 1;
            continue;
        }

        items.push(RenderItem {
            to: relation.to.clone(),
            kind: relation.kind.clone(),
            target_label,
            evidence: evidence_refs(investigation, relation),
            target_count: 1,
        });
    }

    for item in &mut items {
        if item.target_count > 1 && investigation.graph.outgoing(&item.to).next().is_none() {
            item.target_label = format!("{} ({})", item.target_label, item.target_count);
        }
    }

    items
}

fn relation_target_label(investigation: &Investigation, relation: &Relation) -> String {
    investigation
        .graph
        .entity(&relation.to)
        .map(|entity| entity.name.clone())
        .unwrap_or_else(|| relation.to.to_string())
}

fn is_low_signal_systemd_unit(investigation: &Investigation, relation: &Relation) -> bool {
    if !matches!(
        relation.kind,
        RelationKind::Requires | RelationKind::References
    ) {
        return false;
    }

    if !relation
        .evidence
        .iter()
        .any(|evidence| evidence.backend == "systemd")
    {
        return false;
    }

    let Some(entity) = investigation.graph.entity(&relation.to) else {
        return false;
    };

    matches!(
        entity.name.as_str(),
        "-.mount"
            | "basic.target"
            | "init.scope"
            | "local-fs.target"
            | "paths.target"
            | "shutdown.target"
            | "slices.target"
            | "sockets.target"
            | "sysinit.target"
            | "system.slice"
    )
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

fn evidence_refs(investigation: &Investigation, relation: &Relation) -> Vec<EvidenceRef> {
    let mut refs = Vec::new();
    let mut index = 1;

    for existing in investigation.graph.relations() {
        for evidence in &existing.evidence {
            if existing.from == relation.from
                && existing.to == relation.to
                && existing.kind == relation.kind
            {
                refs.push(EvidenceRef {
                    id: format!("e{index}"),
                    confidence: evidence.confidence,
                });
            }
            index += 1;
        }
    }

    refs
}

fn evidence_marker(evidence: &[EvidenceRef]) -> String {
    let Some(first) = evidence.first() else {
        return String::new();
    };

    if evidence.len() == 1 {
        return format!(" [{} {}]", first.id, first.confidence);
    }

    if evidence
        .iter()
        .all(|item| item.confidence == first.confidence)
    {
        return format!(
            " [{} +{} {}]",
            first.id,
            evidence.len() - 1,
            first.confidence
        );
    }

    format!(" [{} +{}]", first.id, evidence.len() - 1)
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn push_section(output: &mut String, title: &str, color: bool) {
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(&styled(title, "92", color));
    output.push('\n');
}

fn styled(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
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

        assert!(output.contains("Query"));
        assert!(output.contains("interpreted as auto search"));
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
        assert!(output.contains("Evidence"));
        assert!(output.contains("Relation: /tmp/link --resolves to--> /tmp/target"));
        assert!(output.contains("Backend: filesystem"));
        assert!(output.contains("Description: Path resolves to this canonical target"));
    }

    #[test]
    fn compact_graph_renders_relation_and_target_on_one_line() {
        let investigation = investigation_with_one_relation();
        let output = plain::render(&investigation, plain::PlainRenderMode::Compact);

        assert!(output.contains("└── resolves to /tmp/target [e1 exact]"));
    }

    #[test]
    fn compact_graph_groups_repeated_leaf_targets() {
        let mut graph = EvidenceGraph::new();
        let store = graph.add_entity(Entity::new(
            EntityId::new("store-path:/nix/store/demo"),
            EntityKind::StorePath,
            "/nix/store/demo",
        ));
        let root_one = graph.add_entity(Entity::new(
            EntityId::new("file:/nix/var/nix/profiles/system-1-link"),
            EntityKind::File,
            "system profile",
        ));
        let root_two = graph.add_entity(Entity::new(
            EntityId::new("file:/nix/var/nix/profiles/system-2-link"),
            EntityKind::File,
            "system profile",
        ));
        for root in [root_one, root_two] {
            graph.add_relation(Relation::new(
                store.clone(),
                root,
                RelationKind::ReachableFrom,
                Confidence::Exact,
                vec![Evidence::new(
                    "nix",
                    "nix-store --query --roots",
                    "Nix reports this root",
                    Confidence::Exact,
                )],
            ));
        }

        let investigation = Investigation {
            query: Query::StorePath("/nix/store/demo".into()),
            answer: "Nix store path found.".to_string(),
            graph,
            matches: vec![store],
            incomplete: Vec::new(),
            backend_status: Vec::new(),
        };
        let output = plain::render(&investigation, plain::PlainRenderMode::Compact);

        assert!(output.contains("kept because of system profile (2) [e1 +1 exact]"));
    }

    #[test]
    fn compact_graph_groups_low_signal_systemd_units() {
        let investigation = investigation_with_systemd_dependencies();
        let output = plain::render(&investigation, plain::PlainRenderMode::Compact);

        assert!(output.contains("requires dbus.socket"));
        assert!(output.contains("requires systemd base units (2)"));
        assert!(!output.contains("requires system.slice"));
        assert!(!output.contains("requires -.mount"));
    }

    #[test]
    fn full_graph_keeps_low_signal_systemd_units() {
        let investigation = investigation_with_systemd_dependencies();
        let output = plain::render(&investigation, plain::PlainRenderMode::Full);

        assert!(output.contains("requires dbus.socket"));
        assert!(output.contains("requires system.slice"));
        assert!(output.contains("requires -.mount"));
    }

    #[test]
    fn full_mode_renders_graph_details() {
        let investigation = investigation_with_one_relation();
        let output = plain::render(&investigation, plain::PlainRenderMode::Full);

        assert!(output.contains("Evidence"));
        assert!(output.contains("Graph"));
        assert!(output.contains("Entities:"));
        assert!(output.contains("Relations:"));
    }

    #[test]
    fn debug_mode_renders_debug_diagnostics() {
        let investigation = investigation_with_one_relation();
        let output = plain::render(&investigation, plain::PlainRenderMode::Debug);

        assert!(output.contains("Graph"));
        assert!(output.contains("Debug"));
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

    fn investigation_with_systemd_dependencies() -> Investigation {
        let mut graph = EvidenceGraph::new();
        let service = graph.add_entity(Entity::new(
            EntityId::new("service:systemd:dbus.service"),
            EntityKind::Service,
            "dbus.service",
        ));

        for dependency in ["dbus.socket", "system.slice", "-.mount"] {
            let dependency_id = graph.add_entity(Entity::new(
                EntityId::new(format!("service:systemd:{dependency}")),
                EntityKind::Service,
                dependency,
            ));
            graph.add_relation(Relation::new(
                service.clone(),
                dependency_id,
                RelationKind::Requires,
                Confidence::Exact,
                vec![Evidence::new(
                    "systemd",
                    "systemctl show dbus.service --property=Requires",
                    "systemd reports this Requires unit relationship",
                    Confidence::Exact,
                )],
            ));
        }

        Investigation {
            query: Query::Service("dbus".to_string()),
            answer: "Service found.".to_string(),
            graph,
            matches: vec![service],
            incomplete: Vec::new(),
            backend_status: Vec::new(),
        }
    }
}
