use serde::Deserialize;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;

const DEFAULT_API_URL: &str = "https://api.cnb.cool";

#[derive(Debug, Default, Deserialize)]
pub struct AuthConfig {
    pub token: Option<String>,
    pub api_url: Option<String>,
}

#[derive(Debug)]
pub struct ResolvedConfig {
    pub token: Option<String>,
    pub api_url: String,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(err) => write!(f, "failed to read config: {err}"),
            ConfigError::Parse(err) => write!(f, "failed to parse config JSON: {err}"),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::Io(err) => Some(err),
            ConfigError::Parse(err) => Some(err),
        }
    }
}

impl AuthConfig {
    /// Load config from XDG config dir: $XDG_CONFIG_HOME/cnb/auth.json
    pub fn load() -> Result<Self, ConfigError> {
        let Some(path) = Self::config_path() else {
            return Ok(Self::default());
        };

        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(err) => return Err(ConfigError::Io(err)),
        };

        serde_json::from_str(&content).map_err(ConfigError::Parse)
    }

    pub fn resolve(
        cli_api_url: Option<&str>,
        cli_token: Option<&str>,
        file: AuthConfig,
    ) -> ResolvedConfig {
        let api_url = cli_api_url
            .map(str::to_string)
            .or(file.api_url)
            .unwrap_or_else(|| DEFAULT_API_URL.to_string());
        let token = cli_token.map(str::to_string).or(file.token);

        ResolvedConfig { token, api_url }
    }

    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("cnb").join("auth.json"))
    }
}
