//! Typed server configuration.
//!
//! Configuration is read once at startup and validated eagerly: an
//! unusable deployment should fail to boot with a clear message rather
//! than fail later on a user request. Secret-bearing values are wrapped
//! so they cannot be printed by accident.

use std::collections::HashMap;
use std::env;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Environment variable names, listed once so `.env.example`, the
/// documentation, and the parser cannot drift apart silently.
pub mod vars {
    pub const DATABASE_URL: &str = "HOMECLOUD_DATABASE_URL";
    pub const DATABASE_MAX_CONNECTIONS: &str = "HOMECLOUD_DATABASE_MAX_CONNECTIONS";
    pub const LISTEN_ADDR: &str = "HOMECLOUD_LISTEN_ADDR";
    pub const STORAGE_ROOT: &str = "HOMECLOUD_STORAGE_ROOT";
    pub const ENVIRONMENT: &str = "HOMECLOUD_ENV";
}

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_STORAGE_ROOT: &str = "./data/library";
const DEFAULT_MAX_CONNECTIONS: u32 = 8;
const MAX_ALLOWED_CONNECTIONS: u32 = 128;

/// Database connection acquisition budget. Bounded so a saturated pool
/// surfaces as a readiness failure instead of an unbounded request stall.
pub const DATABASE_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("required configuration `{0}` is missing")]
    Missing(&'static str),
    #[error("configuration `{name}` is invalid: {reason}")]
    Invalid { name: &'static str, reason: String },
}

/// A configuration value that must never reach logs or error output.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Deployment posture. Development relaxes nothing security-relevant; it
/// only changes log formatting and diagnostics verbosity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Environment {
    #[default]
    Development,
    Production,
}

impl Environment {
    pub fn is_production(self) -> bool {
        matches!(self, Environment::Production)
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Connection string, secret because it usually embeds a password.
    pub url: Secret,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
}

impl DatabaseConfig {
    /// Exposes the connection string for the database driver. Callers
    /// must not log the returned value.
    pub fn database_url(&self) -> &str {
        self.url.expose()
    }
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub listen_addr: SocketAddr,
    pub storage_root: PathBuf,
    pub database: DatabaseConfig,
    pub environment: Environment,
}

/// Where configuration values come from. Tests supply a map instead of
/// mutating process environment, which is global and racy under a
/// multi-threaded test runner.
pub trait ConfigSource {
    fn get(&self, name: &str) -> Option<String>;
}

/// Reads the real process environment.
pub struct EnvSource;

impl ConfigSource for EnvSource {
    fn get(&self, name: &str) -> Option<String> {
        env::var(name).ok()
    }
}

impl ConfigSource for HashMap<String, String> {
    fn get(&self, name: &str) -> Option<String> {
        HashMap::get(self, name).cloned()
    }
}

impl ServerConfig {
    /// Reads and validates configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_source(&EnvSource)
    }

    pub fn from_source(source: &impl ConfigSource) -> Result<Self, ConfigError> {
        let database_url = required(source, vars::DATABASE_URL)?;

        let listen_addr = optional(source, vars::LISTEN_ADDR)
            .unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_owned())
            .parse::<SocketAddr>()
            .map_err(|error| ConfigError::Invalid {
                name: vars::LISTEN_ADDR,
                reason: error.to_string(),
            })?;

        let max_connections = match optional(source, vars::DATABASE_MAX_CONNECTIONS) {
            None => DEFAULT_MAX_CONNECTIONS,
            Some(raw) => {
                let parsed = raw.parse::<u32>().map_err(|error| ConfigError::Invalid {
                    name: vars::DATABASE_MAX_CONNECTIONS,
                    reason: error.to_string(),
                })?;
                if parsed == 0 || parsed > MAX_ALLOWED_CONNECTIONS {
                    return Err(ConfigError::Invalid {
                        name: vars::DATABASE_MAX_CONNECTIONS,
                        reason: format!("must be between 1 and {MAX_ALLOWED_CONNECTIONS}"),
                    });
                }
                parsed
            }
        };

        let storage_root = PathBuf::from(
            optional(source, vars::STORAGE_ROOT).unwrap_or_else(|| DEFAULT_STORAGE_ROOT.to_owned()),
        );

        let environment = match optional(source, vars::ENVIRONMENT).as_deref() {
            None => Environment::default(),
            Some("development") => Environment::Development,
            Some("production") => Environment::Production,
            Some(other) => {
                return Err(ConfigError::Invalid {
                    name: vars::ENVIRONMENT,
                    reason: format!("expected `development` or `production`, got `{other}`"),
                })
            }
        };

        Ok(Self {
            listen_addr,
            storage_root,
            database: DatabaseConfig {
                url: Secret(database_url),
                max_connections,
                acquire_timeout: DATABASE_ACQUIRE_TIMEOUT,
            },
            environment,
        })
    }
}

/// Treats a present-but-blank variable as absent: an empty value in a
/// container environment is nearly always an unset value, and silently
/// accepting it produces a confusing failure much later.
fn optional(source: &impl ConfigSource, name: &str) -> Option<String> {
    source
        .get(name)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required(source: &impl ConfigSource, name: &'static str) -> Result<String, ConfigError> {
    optional(source, name).ok_or(ConfigError::Missing(name))
}
