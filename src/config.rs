//! Configuration loaded from the process environment.

use std::env;

/// Runtime configuration for the MCP server.
#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    /// Internal Semaphore URL used for REST calls.
    pub semaphore_url: String,
    /// Public Semaphore URL used to build user-facing `web_urls`.
    pub public_url: String,
    /// Address the MCP HTTP server binds to.
    pub host: String,
    /// Port the MCP HTTP server binds to.
    pub port: u16,
    /// Optional fixed API token. When set, it takes precedence over the
    /// per-request `Authorization: Bearer` header.
    pub api_token: Option<String>,
}

/// Hand-written so the API token is never printed by logs or panics.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("semaphore_url", &self.semaphore_url)
            .field("public_url", &self.public_url)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("api_token", &self.api_token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Errors produced while loading [`Config`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("SEMAPHORE_URL is required")]
    MissingSemaphoreUrl,
    #[error("invalid value for {var}: {value}")]
    InvalidValue { var: &'static str, value: String },
}

impl Config {
    /// Load the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    /// Load the configuration from an arbitrary lookup function.
    ///
    /// This keeps the parsing logic testable without mutating global state.
    pub fn from_lookup<F>(lookup: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let semaphore_url = lookup("SEMAPHORE_URL")
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty())
            .ok_or(ConfigError::MissingSemaphoreUrl)?;

        let public_url = lookup("SEMAPHORE_PUBLIC_URL")
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| semaphore_url.clone());

        let host = lookup("MCP_HOST")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "0.0.0.0".to_string());

        let port = match lookup("MCP_PORT") {
            Some(value) => value
                .trim()
                .parse::<u16>()
                .map_err(|_| ConfigError::InvalidValue {
                    var: "MCP_PORT",
                    value,
                })?,
            None => 8090,
        };

        let api_token = lookup("SEMAPHORE_API_TOKEN")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        Ok(Self {
            semaphore_url,
            public_url,
            host,
            port,
            api_token,
        })
    }

    /// Resolve the Semaphore token for a request.
    ///
    /// A configured `SEMAPHORE_API_TOKEN` always wins; otherwise the token is
    /// taken from the incoming `Authorization: Bearer <token>` header.
    pub fn resolve_token(&self, authorization: Option<&str>) -> Option<String> {
        if let Some(token) = self.api_token.as_deref().filter(|token| !token.is_empty()) {
            return Some(token.to_string());
        }

        let header = authorization?;
        let (scheme, token) = header.trim().split_once(char::is_whitespace)?;
        // HTTP auth schemes are case-insensitive (RFC 7235).
        if !scheme.eq_ignore_ascii_case("bearer") {
            return None;
        }
        let token = token.trim();
        if token.is_empty() {
            None
        } else {
            Some(token.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn set(vars: &[(&'static str, Option<&str>)]) -> Self {
            let saved = vars
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect();
            for (key, value) in vars {
                // SAFETY: tests serialize environment mutations with ENV_LOCK.
                unsafe {
                    match value {
                        Some(value) => env::set_var(key, value),
                        None => env::remove_var(key),
                    }
                }
            }
            Self { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.saved {
                // SAFETY: tests serialize environment mutations with ENV_LOCK.
                unsafe {
                    match value {
                        Some(value) => env::set_var(key, value),
                        None => env::remove_var(key),
                    }
                }
            }
        }
    }

    #[test]
    fn public_url_defaults_to_semaphore_url() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard::set(&[
            ("SEMAPHORE_URL", Some("https://semaphore.example.com/")),
            ("SEMAPHORE_PUBLIC_URL", None),
            ("SEMAPHORE_API_TOKEN", None),
            ("MCP_HOST", None),
            ("MCP_PORT", None),
        ]);

        let config = Config::from_env().expect("config should load");

        assert_eq!(config.semaphore_url, "https://semaphore.example.com");
        assert_eq!(config.public_url, "https://semaphore.example.com");
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8090);
    }

    #[test]
    fn public_url_uses_explicit_value() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard::set(&[
            ("SEMAPHORE_URL", Some("http://semaphore:3000")),
            (
                "SEMAPHORE_PUBLIC_URL",
                Some("https://semaphore.example.com/ui/"),
            ),
            ("SEMAPHORE_API_TOKEN", None),
            ("MCP_HOST", None),
            ("MCP_PORT", None),
        ]);

        let config = Config::from_env().expect("config should load");

        assert_eq!(config.semaphore_url, "http://semaphore:3000");
        assert_eq!(config.public_url, "https://semaphore.example.com/ui");
    }

    #[test]
    fn missing_semaphore_url_is_rejected() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard::set(&[
            ("SEMAPHORE_URL", None),
            ("SEMAPHORE_PUBLIC_URL", None),
            ("SEMAPHORE_API_TOKEN", None),
            ("MCP_HOST", None),
            ("MCP_PORT", None),
        ]);

        let result = Config::from_env();

        assert!(matches!(result, Err(ConfigError::MissingSemaphoreUrl)));
    }

    #[test]
    fn configured_token_wins_over_bearer_header() {
        let config = Config {
            semaphore_url: "http://semaphore:3000".to_string(),
            public_url: "http://semaphore:3000".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8090,
            api_token: Some("fixed-token".to_string()),
        };

        assert_eq!(
            config.resolve_token(Some("Bearer request-token")),
            Some("fixed-token".to_string())
        );
    }

    #[test]
    fn bearer_header_is_used_without_configured_token() {
        let config = Config {
            semaphore_url: "http://semaphore:3000".to_string(),
            public_url: "http://semaphore:3000".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8090,
            api_token: None,
        };

        assert_eq!(
            config.resolve_token(Some("Bearer request-token")),
            Some("request-token".to_string())
        );
        assert_eq!(config.resolve_token(None), None);
    }

    #[test]
    fn bearer_scheme_is_case_insensitive_and_tolerates_whitespace() {
        let config = Config {
            semaphore_url: "http://semaphore:3000".to_string(),
            public_url: "http://semaphore:3000".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8090,
            api_token: None,
        };

        assert_eq!(
            config.resolve_token(Some("BEARER request-token")),
            Some("request-token".to_string())
        );
        assert_eq!(
            config.resolve_token(Some("bEaReR \t  request-token")),
            Some("request-token".to_string())
        );
        assert_eq!(config.resolve_token(Some("Basic request-token")), None);
        assert_eq!(config.resolve_token(Some("Bearer")), None);
        assert_eq!(config.resolve_token(Some("Bearer   ")), None);
    }

    #[test]
    fn allowed_hosts_defaults_to_loopback_when_unset() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard::set(&[
            ("SEMAPHORE_URL", Some("http://semaphore:3000")),
            ("MCP_ALLOWED_HOSTS", None),
        ]);

        let config = Config::from_env().expect("config should load");

        assert_eq!(config.allowed_hosts, AllowedHosts::Default);
    }

    #[test]
    fn allowed_hosts_star_means_any() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard::set(&[
            ("SEMAPHORE_URL", Some("http://semaphore:3000")),
            ("MCP_ALLOWED_HOSTS", Some(" * ")),
        ]);

        let config = Config::from_env().expect("config should load");

        assert_eq!(config.allowed_hosts, AllowedHosts::Any);
    }

    #[test]
    fn allowed_hosts_parses_comma_separated_list() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard::set(&[
            ("SEMAPHORE_URL", Some("http://semaphore:3000")),
            ("MCP_ALLOWED_HOSTS", Some("a, b ,")),
        ]);

        let config = Config::from_env().expect("config should load");

        assert_eq!(
            config.allowed_hosts,
            AllowedHosts::List(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn allowed_hosts_blank_value_falls_back_to_default() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard::set(&[
            ("SEMAPHORE_URL", Some("http://semaphore:3000")),
            ("MCP_ALLOWED_HOSTS", Some(" , ")),
        ]);

        let config = Config::from_env().expect("config should load");

        assert_eq!(config.allowed_hosts, AllowedHosts::Default);
    }

    #[test]
    fn debug_output_redacts_the_api_token() {
        let config = Config {
            semaphore_url: "http://semaphore:3000".to_string(),
            public_url: "http://semaphore:3000".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8090,
            api_token: Some("super-secret-token".to_string()),
        };

        let debug = format!("{config:?}");

        assert!(debug.contains("<redacted>"), "unexpected debug: {debug}");
        assert!(
            !debug.contains("super-secret-token"),
            "token leaked: {debug}"
        );
    }
}
