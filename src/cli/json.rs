use crate::engine::Investigation;

pub fn render(investigation: &Investigation) -> String {
    let mut output = String::new();
    output.push_str("{\n");
    output.push_str("  \"query\": {\n");
    output.push_str(&format!(
        "    \"kind\": {},\n",
        json_string(investigation.query.kind())
    ));
    output.push_str(&format!(
        "    \"value\": {}\n",
        json_string(&investigation.query.value())
    ));
    output.push_str("  },\n");

    output.push_str("  \"entities\": [");
    if investigation.graph.entities().next().is_some() {
        output.push('\n');
        let entities: Vec<_> = investigation.graph.entities().collect();
        for (index, entity) in entities.iter().enumerate() {
            output.push_str("    {\n");
            output.push_str(&format!(
                "      \"id\": {},\n",
                json_string(entity.id.as_str())
            ));
            output.push_str(&format!(
                "      \"kind\": {},\n",
                json_string(entity.kind.as_str())
            ));
            output.push_str(&format!("      \"name\": {}\n", json_string(&entity.name)));
            output.push_str("    }");
            if index + 1 != entities.len() {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str("  ");
    }
    output.push_str("],\n");

    output.push_str("  \"relations\": [");
    if investigation.graph.relations().next().is_some() {
        output.push('\n');
        let relations: Vec<_> = investigation.graph.relations().collect();
        for (index, relation) in relations.iter().enumerate() {
            output.push_str("    {\n");
            output.push_str(&format!(
                "      \"from\": {},\n",
                json_string(relation.from.as_str())
            ));
            output.push_str(&format!(
                "      \"to\": {},\n",
                json_string(relation.to.as_str())
            ));
            output.push_str(&format!(
                "      \"kind\": {},\n",
                json_string(relation.kind.as_str())
            ));
            output.push_str(&format!(
                "      \"confidence\": {}\n",
                json_string(relation.confidence.as_str())
            ));
            output.push_str("    }");
            if index + 1 != relations.len() {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str("  ");
    }
    output.push_str("],\n");

    output.push_str("  \"incomplete\": [");
    if !investigation.incomplete.is_empty() {
        output.push('\n');
        for (index, message) in investigation.incomplete.iter().enumerate() {
            output.push_str(&format!("    {}", json_string(message)));
            if index + 1 != investigation.incomplete.len() {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str("  ");
    }
    output.push_str("],\n");

    output.push_str("  \"backend_status\": [");
    if !investigation.backend_status.is_empty() {
        output.push('\n');
        for (index, status) in investigation.backend_status.iter().enumerate() {
            output.push_str("    {\n");
            output.push_str(&format!(
                "      \"backend\": {},\n",
                json_string(&status.backend)
            ));
            output.push_str(&format!(
                "      \"status\": {}\n",
                json_string(status.state.as_display())
            ));
            output.push_str("    }");
            if index + 1 != investigation.backend_status.len() {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str("  ");
    }
    output.push_str("]\n");
    output.push_str("}\n");

    output
}

fn json_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use crate::cli::json;
    use crate::core::Query;
    use crate::engine::Investigation;

    #[test]
    fn renders_empty_phase_zero_investigation_as_json() {
        let investigation = Investigation::empty(Query::Auto("firefox".to_string()));
        let output = json::render(&investigation);

        assert!(output.contains("\"kind\": \"auto\""));
        assert!(output.contains("\"value\": \"firefox\""));
        assert!(output.contains("\"entities\": []"));
        assert!(output.contains("\"relations\": []"));
        assert!(output.contains("\"backend\": \"filesystem\""));
    }

    #[test]
    fn escapes_json_strings() {
        assert_eq!(super::json_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
