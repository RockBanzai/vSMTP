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

use vsmtp_config::{logs, semver, Broker, Config, Logs};

pub mod cli;

/// Configuration for the SMTP receiver.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkingConfig {
    pub api_version: semver::VersionReq,
    /// Name of the server. Used when identifying itself to the client.
    #[serde(default = "WorkingConfig::default_name")]
    pub name: String,
    /// Filters configuration.
    #[serde(default)]
    pub scripts: Scripts,
    /// AMQP client configuration.
    #[serde(default)]
    pub broker: Broker,
    /// logging configuration.
    #[serde(default)]
    pub logs: Logs,
    /// Path to the configuration script.
    #[serde(skip)]
    pub path: std::path::PathBuf,
}

impl WorkingConfig {
    fn default_name() -> String {
        "vsmtp".to_string()
    }
}

/// Scripts location and parameters.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scripts {
    #[serde(default = "Scripts::default_script_path")]
    pub path: std::path::PathBuf,
}

impl Default for Scripts {
    fn default() -> Self {
        Self {
            path: Self::default_script_path(),
        }
    }
}

impl Scripts {
    fn default_script_path() -> std::path::PathBuf {
        <std::path::PathBuf as std::str::FromStr>::from_str("/etc/vsmtp/working/script.rhai")
            .expect("infallible")
    }
}

impl Config for WorkingConfig {
    fn with_path(&mut self, path: &impl AsRef<std::path::Path>) {
        self.path = path.as_ref().into();
    }

    fn api_version(&self) -> &semver::VersionReq {
        &self.api_version
    }

    fn broker(&self) -> &Broker {
        &self.broker
    }

    fn logs(&self) -> &logs::Logs {
        &self.logs
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}
