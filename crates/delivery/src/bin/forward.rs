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

use std::sync::Arc;
use vsmtp_common::{
    ctx_delivery::CtxDelivery, delivery_attempt::DeliveryAttempt, delivery_route::DeliveryRoute,
};
use vsmtp_config::Config;
use vsmtp_delivery::{delivery_main, smtp::send, DeliverySystem, ShouldNotify};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Forward {
    service: String,
    target: url::Url,
    api_version: vsmtp_config::semver::VersionReq,
    #[serde(default)]
    queues: vsmtp_config::Queues,
    #[serde(default)]
    broker: vsmtp_config::Broker,
    #[serde(default)]
    logs: vsmtp_config::Logs,
    #[serde(skip)]
    path: std::path::PathBuf,
}

#[async_trait::async_trait]
impl DeliverySystem for Forward {
    fn routing_key(&self) -> DeliveryRoute {
        DeliveryRoute::Forward {
            service: self.service.clone(),
        }
    }

    fn get_notification_supported() -> ShouldNotify {
        ShouldNotify {
            on_success: false,
            on_failure: true,
            on_delay: true,
        }
    }

    async fn deliver(
        self: Arc<Self>,
        CtxDelivery {
            uuid: _,
            routing_key: _,
            mail_from,
            rcpt_to,
            mail,
            attempt: _,
        }: &CtxDelivery,
    ) -> Vec<DeliveryAttempt> {
        let message_str = mail.read().unwrap().to_string();

        assert!(self.target.scheme() == "smtp");

        vec![
            send(
                self.target.host_str().unwrap(),
                self.target.port().unwrap_or(25),
                &hostname::get().unwrap().to_string_lossy(),
                mail_from.clone(),
                rcpt_to.clone(),
                message_str.as_bytes(),
            )
            .await,
        ]
    }
}

impl Config for Forward {
    fn with_path(_: &impl AsRef<std::path::Path>) -> vsmtp_config::ConfigResult<Self> {
        Ok(Self {
            target: url::Url::parse("smtp://localhost").unwrap(),
            service: String::default(),
            api_version: vsmtp_config::semver::VersionReq::default(),
            queues: vsmtp_config::Queues::default(),
            broker: vsmtp_config::Broker::default(),
            logs: vsmtp_config::Logs::default(),
            path: std::path::PathBuf::default(),
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

#[derive(clap::Parser)]
#[command(author, version, about)]
struct Args {
    /// Path to the rhai configuration file.
    #[arg(short, long, default_value_t = String::from("/etc/vsmtp/forward/conf.d/config.rhai"))]
    pub config: String,
}

#[tokio::main]
async fn main() {
    let Args { config } = <Args as clap::Parser>::parse();

    let system = match Forward::from_rhai_file(&config) {
        Ok(cfg) => std::sync::Arc::new(cfg),
        Err(error) => {
            eprintln!("Failed to initialize forward delivery configuration: {error}");
            return;
        }
    };

    if let Err(error) = delivery_main(system).await {
        tracing::error!("Failed to run forward delivery: {error}");
    }
}
