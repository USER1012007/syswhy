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

    let output = match output_mode {
        OutputMode::Plain => plain::render(&investigation, false),
        OutputMode::Evidence | OutputMode::Full | OutputMode::Debug => {
            plain::render(&investigation, true)
        }
        OutputMode::Json => json::render(&investigation),
    };

    print!("{output}");
}
