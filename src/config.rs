//! Configuration loaded from the process environment.

use std::env;

/// Runtime configuration for the MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
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

        authorization
            .and_then(|header| {
                header
                    .strip_prefix("Bearer ")
                    .or_else(|| header.strip_prefix("bearer "))
            })
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string)
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
}
