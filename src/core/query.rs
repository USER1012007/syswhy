use std::path::{Path, PathBuf};

use crate::core::SyswhyError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    Auto(String),
    File(PathBuf),
    Package(String),
    Process(u32),
    Service(String),
    Port {
        number: u16,
        protocol: Option<Protocol>,
    },
    StorePath(PathBuf),
}

impl Query {
    pub fn parse(parts: &[String]) -> Result<Self, SyswhyError> {
        if parts.is_empty() {
            return Err(SyswhyError::InvalidQuery(
                "expected a query, for example: syswhy firefox".to_string(),
            ));
        }

        match parts[0].as_str() {
            "package" => {
                let value = single_value("package", parts)?;
                Ok(Self::Package(value.to_string()))
            }
            "pid" | "process" => {
                let value = single_value(parts[0].as_str(), parts)?;
                let pid = value
                    .parse::<u32>()
                    .map_err(|_| SyswhyError::InvalidQuery(format!("invalid PID: {value}")))?;
                Ok(Self::Process(pid))
            }
            "service" => {
                let value = single_value("service", parts)?;
                Ok(Self::Service(value.to_string()))
            }
            "port" => {
                let value = single_value("port", parts)?;
                parse_port(value)
            }
            _ => {
                let raw = parts.join(" ");
                if is_nix_store_path(&raw) {
                    Ok(Self::StorePath(PathBuf::from(raw)))
                } else if looks_like_path(&raw) {
                    Ok(Self::File(PathBuf::from(raw)))
                } else {
                    Ok(Self::Auto(raw))
                }
            }
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Auto(_) => "auto",
            Self::File(_) => "file",
            Self::Package(_) => "package",
            Self::Process(_) => "process",
            Self::Service(_) => "service",
            Self::Port { .. } => "port",
            Self::StorePath(_) => "store_path",
        }
    }

    pub fn value(&self) -> String {
        match self {
            Self::Auto(value) | Self::Package(value) | Self::Service(value) => value.clone(),
            Self::File(path) | Self::StorePath(path) => path.display().to_string(),
            Self::Process(pid) => pid.to_string(),
            Self::Port { number, protocol } => match protocol {
                Some(protocol) => format!("{}:{number}", protocol.as_str()),
                None => number.to_string(),
            },
        }
    }

    pub fn interpreted_as(&self) -> &'static str {
        match self {
            Self::Auto(_) => "auto search",
            Self::File(_) => "file path",
            Self::Package(_) => "package search",
            Self::Process(_) => "process id",
            Self::Service(_) => "systemd service",
            Self::Port { .. } => "network port",
            Self::StorePath(_) => "nix store path",
        }
    }
}

fn single_value<'a>(kind: &str, parts: &'a [String]) -> Result<&'a str, SyswhyError> {
    if parts.len() != 2 {
        return Err(SyswhyError::InvalidQuery(format!(
            "{kind} queries require exactly one value"
        )));
    }

    Ok(parts[1].as_str())
}

fn parse_port(value: &str) -> Result<Query, SyswhyError> {
    let (protocol, number) = if let Some((protocol, number)) = value.split_once(':') {
        let protocol = match protocol {
            "tcp" => Protocol::Tcp,
            "udp" => Protocol::Udp,
            other => {
                return Err(SyswhyError::InvalidQuery(format!(
                    "unsupported port protocol: {other}"
                )));
            }
        };

        (Some(protocol), number)
    } else {
        (None, value)
    };

    let number = number
        .parse::<u16>()
        .map_err(|_| SyswhyError::InvalidQuery(format!("invalid port: {number}")))?;

    Ok(Query::Port { number, protocol })
}

fn looks_like_path(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute() || value.starts_with("./") || value.starts_with("../") || value == "."
}

fn is_nix_store_path(value: &str) -> bool {
    value == "/nix/store" || value.starts_with("/nix/store/")
}

#[cfg(test)]
mod tests {
    use super::{Protocol, Query};

    fn parts(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_auto_query() {
        assert_eq!(
            Query::parse(&parts(&["firefox"])).unwrap(),
            Query::Auto("firefox".to_string())
        );
    }

    #[test]
    fn parses_file_query() {
        assert_eq!(
            Query::parse(&parts(&["/usr/bin/bash"])).unwrap(),
            Query::File("/usr/bin/bash".into())
        );
    }

    #[test]
    fn parses_store_path_query() {
        assert_eq!(
            Query::parse(&parts(&["/nix/store/abc-firefox"])).unwrap(),
            Query::StorePath("/nix/store/abc-firefox".into())
        );
    }

    #[test]
    fn parses_structured_queries() {
        assert_eq!(
            Query::parse(&parts(&["package", "firefox"])).unwrap(),
            Query::Package("firefox".to_string())
        );
        assert_eq!(
            Query::parse(&parts(&["pid", "42"])).unwrap(),
            Query::Process(42)
        );
        assert_eq!(
            Query::parse(&parts(&["service", "bluetooth"])).unwrap(),
            Query::Service("bluetooth".to_string())
        );
        assert_eq!(
            Query::parse(&parts(&["port", "udp:53317"])).unwrap(),
            Query::Port {
                number: 53317,
                protocol: Some(Protocol::Udp)
            }
        );
    }
}
