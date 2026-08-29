//! Configuration is a security boundary: it decides where files are read
//! from, what the server binds to, and which secrets exist. These tests
//! pin the behaviour a misconfigured deployment must produce.

use std::collections::HashMap;

use homecloud_api::config::{vars, ConfigError, Environment, ServerConfig};

fn source(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn minimal() -> HashMap<String, String> {
    source(&[(
        vars::DATABASE_URL,
        "postgres://homecloud@127.0.0.1:5432/homecloud",
    )])
}

#[test]
fn missing_database_url_is_rejected() {
    let error =
        ServerConfig::from_source(&source(&[])).expect_err("must not start without a database");

    assert_eq!(error, ConfigError::Missing(vars::DATABASE_URL));
}

#[test]
fn blank_database_url_is_treated_as_missing() {
    let error = ServerConfig::from_source(&source(&[(vars::DATABASE_URL, "   ")]))
        .expect_err("blank is not a value");

    assert_eq!(error, ConfigError::Missing(vars::DATABASE_URL));
}

#[test]
fn invalid_listen_address_is_rejected() {
    let mut env = minimal();
    env.insert(vars::LISTEN_ADDR.to_owned(), "not-an-address".to_owned());

    let error = ServerConfig::from_source(&env).expect_err("address must parse");

    assert!(matches!(error, ConfigError::Invalid { name, .. } if name == vars::LISTEN_ADDR));
}

#[test]
fn defaults_are_safe() {
    let config = ServerConfig::from_source(&minimal()).expect("minimal config is valid");

    // Loopback by default: exposure to a network is an explicit decision.
    assert!(config.listen_addr.ip().is_loopback());
    assert_eq!(config.environment, Environment::Development);
    assert!(config.database.max_connections > 0);
    assert!(!config.database.acquire_timeout.is_zero());
}

#[test]
fn connection_pool_bounds_are_enforced() {
    for invalid in ["0", "1000"] {
        let mut env = minimal();
        env.insert(
            vars::DATABASE_MAX_CONNECTIONS.to_owned(),
            invalid.to_owned(),
        );

        let error = ServerConfig::from_source(&env).expect_err("pool size must be bounded");

        assert!(
            matches!(error, ConfigError::Invalid { name, .. } if name == vars::DATABASE_MAX_CONNECTIONS)
        );
    }
}

#[test]
fn unknown_environment_is_rejected() {
    let mut env = minimal();
    env.insert(vars::ENVIRONMENT.to_owned(), "staging".to_owned());

    assert!(ServerConfig::from_source(&env).is_err());
}

#[test]
fn production_environment_is_recognised() {
    let mut env = minimal();
    env.insert(vars::ENVIRONMENT.to_owned(), "production".to_owned());

    let config = ServerConfig::from_source(&env).expect("valid config");

    assert!(config.environment.is_production());
}

#[test]
fn debug_output_never_contains_the_database_password() {
    let mut env = minimal();
    env.insert(
        vars::DATABASE_URL.to_owned(),
        "postgres://homecloud:sup3r-s3cret@db:5432/homecloud".to_owned(),
    );

    let config = ServerConfig::from_source(&env).expect("valid config");
    let rendered = format!("{config:?}");

    assert!(
        !rendered.contains("sup3r-s3cret"),
        "secret leaked: {rendered}"
    );
    assert!(rendered.contains("redacted"));
    assert_eq!(
        config.database.url.expose(),
        "postgres://homecloud:sup3r-s3cret@db:5432/homecloud"
    );
}
