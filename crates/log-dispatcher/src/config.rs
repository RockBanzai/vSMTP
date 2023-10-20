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

/// Available formatters for logs
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// syslog formatter https://www.rfc-editor.org/rfc/rfc3164
    Rfc3164,
    /// syslog formatter https://www.rfc-editor.org/rfc/rfc5424
    #[default]
    RFC5424,
}

/// Available protocol for syslog logger
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum SyslogProtocol {
    /// Connect to syslog via Udp
    #[default]
    #[serde(rename = "INET_STREAM")]
    Udp,
    /// Connect to syslog via Tcp
    #[serde(rename = "INET_DGRAM")]
    Tcp,
    /// Connect to syslog via Unix socket (datagram)
    #[serde(rename = "UNIX_DGRAM")]
    UnixSocket,
    /// Connect to syslog via Unix stream
    #[serde(rename = "UNIX_STREAM")]
    UnixSocketStream,
}

/// Rotation available for a log file
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRotation {
    /// Rotate every minutes
    Minutely,
    /// Rotate every hours
    Hourly,
    /// Rotate every days
    Daily,
    /// Never rotate the file
    #[default]
    Never,
}

/// Type of logger available
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LogInstanceType {
    /// Send logs to console
    Console {
        /// Formatter used, none by default
        #[serde(default)]
        formatter: Option<LogFormat>,
    },
    /// Send logs to log files
    File {
        /// Folder in which log files are created.
        folder: String,
        /// Prefix used in front of the file, "vsmtp-log" by default.
        #[serde(default = "LogDispatcherConfig::default_file_prefix")]
        file_prefix: String,
        /// Rotation of logs in the files, no rotation by default.
        #[serde(default)]
        rotation: FileRotation,
    },
    /// Send logs to a syslog service
    Syslog {
        /// Formatter used, rfc 5424 by default.
        #[serde(default)]
        formatter: LogFormat,
        /// Protocol used, udp by default.
        #[serde(default)]
        protocol: SyslogProtocol,
        /// Address of the syslog service.
        #[serde(default = "LogDispatcherConfig::default_syslog_address")]
        address: Box<str>,
    },
    /// Send logs to journald
    Journald,
}

/// Configuration of a logger
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LogInstance {
    /// Name of the topic on which the logger will listen.
    pub topic: String,
    /// configuration of the logger which listen to the topic.
    #[serde(flatten)]
    pub config: LogInstanceType,
}

/// Configuration for log dispatcher service.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogDispatcherConfig {
    /// api version supported
    pub api_version: vsmtp_config::semver::VersionReq,
    /// RabbitMQ client configuration.
    #[serde(default)]
    pub broker: vsmtp_config::Broker,
    /// logging configuration.
    #[serde(default)]
    pub logs: vsmtp_config::Logs,
    #[serde(skip)]
    /// Path to the configuration script.
    pub path: std::path::PathBuf,
    /// all loggers used by the log dispatcher.
    #[serde(default, with = "loggers")]
    pub loggers: std::collections::HashMap<String, Vec<LogInstanceType>>,
}

mod loggers {
    use super::{LogInstance, LogInstanceType};

    pub fn serialize<S>(
        value: &std::collections::HashMap<String, Vec<LogInstanceType>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut vec = Vec::new();
        for (topic, instances) in value {
            for instance in instances {
                vec.push(LogInstance {
                    topic: topic.clone(),
                    config: instance.clone(),
                });
            }
        }
        serde::Serialize::serialize(&vec, serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<std::collections::HashMap<String, Vec<LogInstanceType>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <Vec<LogInstance> as serde::Deserialize>::deserialize(deserializer).map(|v| {
            let mut map = std::collections::HashMap::new();
            for instance in v {
                map.entry(instance.topic)
                    .or_insert_with(Vec::new)
                    .push(instance.config);
            }
            map
        })
    }
}

impl LogDispatcherConfig {
    fn default_syslog_address() -> Box<str> {
        "udp://localhost:514".into()
    }

    fn default_file_prefix() -> String {
        "vsmtp-log".to_string()
    }
}

impl Config for LogDispatcherConfig {
    fn with_path(&mut self, path: &impl AsRef<std::path::Path>) {
        self.path = path.as_ref().into();
    }

    fn api_version(&self) -> &vsmtp_config::semver::VersionReq {
        &self.api_version
    }

    fn broker(&self) -> &vsmtp_config::Broker {
        &self.broker
    }

    fn logs(&self) -> &vsmtp_config::logs::Logs {
        &self.logs
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}
