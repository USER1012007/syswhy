use syswhy::prelude::*;

fn main() -> Result<(), SyswhyError> {
    let query_parts = vec!["service".to_string(), "dbus".to_string()];
    let query = Query::parse(&query_parts)?;
    let investigation = Engine::new().investigate(query);

    println!("{}", render_plain(&investigation, PlainRenderMode::Compact));

    for match_id in &investigation.matches {
        if let Some(entity) = investigation.graph.entity(match_id) {
            eprintln!("matched {} {}", entity.kind.as_str(), entity.name);
        }
    }

    Ok(())
}
