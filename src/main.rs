use std::io::IsTerminal;

use syswhy::cli::plain::PlainRenderMode;
use syswhy::cli::{CliArgs, OutputMode, json, plain};
use syswhy::engine::Engine;

fn main() {
    let args = match CliArgs::parse_env() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let output_mode = args.output_mode;
    let investigation = Engine::new().investigate(args.query);
    let color = std::io::stdout().is_terminal();

    let output = match output_mode {
        OutputMode::Plain => {
            plain::render_with_color(&investigation, PlainRenderMode::Compact, color)
        }
        OutputMode::Evidence => {
            plain::render_with_color(&investigation, PlainRenderMode::Evidence, color)
        }
        OutputMode::Full => plain::render_with_color(&investigation, PlainRenderMode::Full, color),
        OutputMode::Debug => {
            plain::render_with_color(&investigation, PlainRenderMode::Debug, color)
        }
        OutputMode::Json => json::render(&investigation),
    };

    print!("{output}");
}
