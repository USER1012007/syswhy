use std::env;

use crate::core::{Query, SyswhyError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Plain,
    Evidence,
    Full,
    Debug,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub query: Query,
    pub output_mode: OutputMode,
}

impl CliArgs {
    pub fn parse_env() -> Result<Self, SyswhyError> {
        Self::parse(env::args().skip(1))
    }

    pub fn parse<I, S>(args: I) -> Result<Self, SyswhyError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut output_mode = OutputMode::Plain;
        let mut query_parts = Vec::new();

        for arg in args {
            let arg = arg.into();
            match arg.as_str() {
                "--plain" => output_mode = OutputMode::Plain,
                "--evidence" => output_mode = OutputMode::Evidence,
                "--full" => output_mode = OutputMode::Full,
                "--debug" => output_mode = OutputMode::Debug,
                "--json" => output_mode = OutputMode::Json,
                "--help" | "-h" => {
                    return Err(SyswhyError::InvalidQuery(Self::usage().to_string()));
                }
                _ if arg.starts_with("--") => {
                    return Err(SyswhyError::InvalidQuery(format!("unknown option: {arg}")));
                }
                _ => query_parts.push(arg),
            }
        }

        Ok(Self {
            query: Query::parse(&query_parts)?,
            output_mode,
        })
    }

    pub fn usage() -> &'static str {
        "usage: syswhy [--plain|--evidence|--full|--debug|--json] <query...>"
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{CliArgs, OutputMode};
    use crate::core::Query;

    #[test]
    fn parses_output_mode_and_query() {
        let args = CliArgs::parse(["--json", "firefox"]).unwrap();

        assert_eq!(args.output_mode, OutputMode::Json);
        assert_eq!(args.query, Query::Auto("firefox".to_string()));
    }

    #[test]
    fn rejects_unknown_options() {
        let error = CliArgs::parse(["--wat", "firefox"]).unwrap_err();

        assert!(error.to_string().contains("unknown option"));
    }
}
