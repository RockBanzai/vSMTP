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

pub(crate) use vsmtp_config::Config;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LogTopic {
    Console {
        formatter: String,
    },
    File {
        file: String,
        formatter: String,
        // TODO: missing rotation
    },
    Syslog {
        formatter: String,
        address: std::net::IpAddr, // FIXME: surely not the right type
                                   // TODO: missing protocol
    },
    Journald {
        formatter: String,
    },
}

/// Configuration for log dispatcher service.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogDispatcherConfig {
    /// api version supported
    pub api_version: vsmtp_config::semver::VersionReq,
    /// Name of the server. Used when identifying itself to the client.
    #[serde(default = "LogDispatcherConfig::default_name")]
    pub name: String,
    /// Queue names to redirect or forward the email.
    pub queues: vsmtp_config::Queues,
    /// RabbitMQ client configuration.
    #[serde(default)]
    pub broker: vsmtp_config::Broker,
    /// logging configuration.
    #[serde(default)]
    pub logs: vsmtp_config::Logs,
    #[serde(skip)]
    /// Path to the configuration script.
    pub path: std::path::PathBuf,
    #[serde(default)] // FIXME: should be deserialized from "type" field
    /// topics on which logs are written.
    pub topics: Vec<LogTopic>,
}

impl LogDispatcherConfig {
    fn default_name() -> String {
        "log-dispatcher".to_string()
    }
}

impl Config for LogDispatcherConfig {
    fn with_path(path: &impl AsRef<std::path::Path>) -> vsmtp_config::ConfigResult<Self>
    where
        Self: vsmtp_config::Config + serde::de::DeserializeOwned + serde::Serialize,
    {
        Ok(Self {
            path: path.as_ref().into(),
            name: LogDispatcherConfig::default_name(),
            ..Default::default()
        })
    }

    fn api_version(&self) -> &vsmtp_config::semver::VersionReq {
        &self.api_version
    }

    fn broker(&self) -> &vsmtp_config::Broker {
        &self.broker
    }

    fn queues(&self) -> &vsmtp_config::Queues {
        &self.queues
    }

    fn logs(&self) -> &vsmtp_config::logs::Logs {
        &self.logs
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}
