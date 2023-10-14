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
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "formatter", rename_all = "lowercase")]
pub enum LogFormat {
    /// syslog formatter https://www.rfc-editor.org/rfc/rfc3164
    Rfc3164,
    /// syslog formatter https://www.rfc-editor.org/rfc/rfc5424
    RFC5424,
}

/// Available protocol for syslog logger
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "protocol")]
pub enum SyslogProtocol {
    /// Connect to syslog via Udp
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
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "rotation", rename_all = "lowercase")]
pub enum FileRotation {
    /// Rotate every minutes
    Minutely,
    /// Rotate every hours
    Hourly,
    /// Rotate every days
    Daily,
    /// Never rotate the file
    Never,
}

/// Type of logger available
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LogInstanceType {
    /// Send logs to console
    Console {
        /// Formatter used, none by default
        #[serde(default = "LogDispatcherConfig::default_log_format", flatten)]
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
        #[serde(default = "LogDispatcherConfig::default_file_rotation")]
        rotation: FileRotation,
    },
    /// Send logs to a syslog service
    Syslog {
        /// Formatter used, rfc 5424 by default.
        #[serde(default = "LogDispatcherConfig::default_syslog_rfc", flatten)]
        formatter: Option<LogFormat>,
        /// Protocol used, udp by default.
        #[serde(default = "LogDispatcherConfig::default_syslog_protocol", flatten)]
        protocol: SyslogProtocol,
        /// Address of the syslog service.
        #[serde(default = "LogDispatcherConfig::default_syslog_address")]
        address: String,
    },
    /// Send logs to journald
    Journald,
}

/// Configuration of a logger
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
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
    /// Name of the server. Used when identifying itself to the client.
    #[serde(default = "LogDispatcherConfig::default_name")]
    pub name: String,
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
    /// all loggers used by the log dispatcher.
    pub loggers: Vec<LogInstance>,
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
    const fn default_syslog_rfc() -> Option<LogFormat> {
        Some(LogFormat::RFC5424)
    }

    #[allow(dead_code)]
    const fn default_log_format() -> Option<LogFormat> {
        None
    }

    #[allow(dead_code)]
    const fn default_file_rotation() -> FileRotation {
        FileRotation::Never
    }

    fn default_syslog_address() -> String {
        "udp://localhost:514".to_string()
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
