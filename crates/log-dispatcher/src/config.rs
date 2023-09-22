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

pub use vsmtp_config::Config;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "formatter", rename_all = "lowercase")]
pub enum LogFormat {
    Full,
    Compact,
    Pretty,
    Json,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "formatter", rename_all = "lowercase")]
pub enum SyslogRfc {
    RFC5424,
    RFC3164,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "protocol")]
pub enum SyslogProtocol {
    #[serde(rename = "INET_STREAM")]
    Udp,
    #[serde(rename = "INET_DGRAM")]
    Tcp,
    #[serde(rename = "UNIX_DGRAM")]
    UnixSocket,
    #[serde(rename = "UNIX_STREAM")]
    UnixSocketStream,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "rotation", rename_all = "lowercase")]
pub enum FileRotation {
    Minutely,
    Hourly,
    Daily,
    Never,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LogTopic {
    Console {
        #[serde(default = "LogDispatcherConfig::default_log_format", flatten)]
        formatter: LogFormat,
    },
    File {
        folder: String,
        #[serde(default = "LogDispatcherConfig::default_file_rotation", flatten)]
        rotation: FileRotation,
    },
    Syslog {
        #[serde(default = "LogDispatcherConfig::default_syslog_rfc", flatten)]
        formatter: SyslogRfc,
        #[serde(default = "LogDispatcherConfig::default_syslog_protocol", flatten)]
        protocol: SyslogProtocol,
        #[serde(default = "LogDispatcherConfig::default_syslog_address")]
        address: String,
    },
    Journald,
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

    #[allow(dead_code)]
    const fn default_syslog_protocol() -> SyslogProtocol {
        SyslogProtocol::Udp
    }

    #[allow(dead_code)]
    const fn default_syslog_rfc() -> SyslogRfc {
        SyslogRfc::RFC5424
    }

    #[allow(dead_code)]
    const fn default_log_format() -> LogFormat {
        LogFormat::Compact
    }

    #[allow(dead_code)]
    const fn default_file_rotation() -> FileRotation {
        FileRotation::Never
    }

    fn default_syslog_address() -> String {
        "udp://localhost:514".to_string()
    }
}

impl Config for LogDispatcherConfig {
    fn with_path(path: &impl AsRef<std::path::Path>) -> vsmtp_config::ConfigResult<Self>
    where
        Self: vsmtp_config::Config + serde::de::DeserializeOwned + serde::Serialize,
    {
        Ok(Self {
            path: path.as_ref().into(),
            name: Self::default_name(),
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
