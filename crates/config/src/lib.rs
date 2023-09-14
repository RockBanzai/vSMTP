/*
 * vSMTP mail transfer agent
 *
 * Copyright (C) 2003 - viridIT SAS
 * Licensed under the Elastic License 2.0
 *
 * You should have received a copy of the Elastic License 2.0 along with
 * this program. If not, see https://www.elastic.co/licensing/elastic-license.
 *
 */

pub mod broker;
pub mod error;
pub mod logs;
pub mod queues;

pub use broker::Broker;
pub use error::ConfigError;
pub use logs::{Logs, LogsFacility, SyslogTransport};
pub use queues::Queues;
pub use semver;

/// Result type for configuration operations.
pub type ConfigResult<T> = Result<T, error::ConfigError>;

/// Getters for base configuration structures.
pub trait Config: serde::Serialize + serde::de::DeserializeOwned + Sized {
    /// Create a default configuration with the path of the script passed
    /// as parameter.
    ///
    /// This function provide the Rhai context with the returned configuration.
    /// Prefer to set any defaults in this function before it can be set by the
    /// user.
    fn with_path(path: &impl AsRef<std::path::Path>) -> ConfigResult<Self>;

    /// Create a configuration structure from a rhai file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the rhai script to create the configuration from.
    ///            The resolve path used is the parent of directory of this path.
    fn from_rhai_file(path: &impl AsRef<std::path::Path>) -> ConfigResult<Self> {
        let path_ref = path.as_ref();

        let config_dir = std::path::PathBuf::from(
            path_ref
                .parent()
                .ok_or_else(|| error::ConfigError::InvalidParentDirectory(path_ref.into()))?,
        );

        let script = std::fs::read_to_string(path_ref)
            .map_err(|error| error::ConfigError::FileOpen(path_ref.into(), error))?;

        Self::from_rhai_script(path, script, Some(&config_dir))
    }

    /// Create a configuration structure from a rhai script.
    ///
    /// # Arguments
    ///
    /// * `script` - The rhai script to use to generate the config.
    /// * `resolve_path` - Path to resolve modules from.
    fn from_rhai_script(
        path: &impl AsRef<std::path::Path>,
        script: impl AsRef<str>,
        resolve_path: Option<&std::path::PathBuf>,
    ) -> ConfigResult<Self> {
        let script = script.as_ref();
        let mut engine = rhai::Engine::new();

        if let Some(resolve_path) = resolve_path.as_ref() {
            engine.set_module_resolver(
                rhai::module_resolvers::FileModuleResolver::new_with_path_and_extension(
                    resolve_path,
                    "rhai",
                ),
            );
        }

        let ast = engine.compile(script)?;

        let cfg = Self::with_path(path)?;
        let cfg = serde_json::to_string(&cfg)?;
        let cfg = rhai::Engine::new().parse_json(cfg, true)?;
        let cfg =
            engine.call_fn::<rhai::Map>(&mut rhai::Scope::new(), &ast, "on_config", (cfg,))?;
        let cfg = serde_json::to_string(&cfg)?;
        let mut cfg = serde_json::Deserializer::from_str(&cfg);
        Ok(serde_path_to_error::deserialize(&mut cfg)?)
    }

    /// The JSON API version to use to communicate with the current service.
    fn api_version(&self) -> &semver::VersionReq;

    /// Broker (`AMQP`) parameters configuration.
    fn broker(&self) -> &broker::Broker;

    /// Special queues used when problems are raised. (quarantine, no-route, etc.)
    fn queues(&self) -> &queues::Queues;

    /// Log configuration for this specific service.
    fn logs(&self) -> &logs::Logs;

    /// Path on disk of the configuration file.
    fn path(&self) -> &std::path::Path;
}

/// Deserialize a certificate from a path into the content of it's content.
pub fn deserialize_certificate<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let path = &<Option<std::path::PathBuf> as serde::Deserialize>::deserialize(deserializer)?;

    Ok(match path {
        Some(path) => Some(std::fs::read_to_string(path).map_err(serde::de::Error::custom)?),
        None => None,
    })
}
