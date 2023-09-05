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

#[derive(clap::Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Path to the rhai configuration file.
    #[arg(short, long, default_value_t = String::from("/etc/vsmtp/delivery/conf.d/config.rhai"))]
    pub config: String,
}

/// Configuration for the delivery service.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct DeliveryConfig {
    /// api version supported
    pub api_version: vsmtp_config::semver::VersionReq,
    /// Name of the server. Used when identifying itself to the client.
    #[serde(default = "DeliveryConfig::default_name")]
    pub name: String,
    /// Queue names to redirect or forward the email.
    #[serde(default)]
    pub queues: vsmtp_config::Queues,
    /// RabbitMQ client configuration.
    #[serde(default)]
    pub broker: vsmtp_config::Broker,
    /// logging configuration.
    #[serde(default)]
    pub logs: vsmtp_config::Logs,
    /// Path to the configuration script.
    #[serde(skip)]
    pub path: std::path::PathBuf,
}

impl DeliveryConfig {
    fn default_name() -> String {
        "delivery".to_string()
    }
}

impl Config for DeliveryConfig {
    fn with_path(path: &impl AsRef<std::path::Path>) -> vsmtp_config::ConfigResult<Self>
    where
        Self: vsmtp_config::Config + serde::de::DeserializeOwned + serde::Serialize,
    {
        Ok(Self {
            path: path.as_ref().into(),
            name: DeliveryConfig::default_name(),
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
