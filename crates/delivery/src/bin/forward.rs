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
use vsmtp_auth::TlsCertificate;
use vsmtp_common::{
    ctx_delivery::CtxDelivery, delivery_attempt::DeliveryAttempt, delivery_route::DeliveryRoute,
};
use vsmtp_config::Config;
use vsmtp_delivery::{delivery_main, smtp::send, DeliverySystem, Tls};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Forward {
    api_version: vsmtp_config::semver::VersionReq,
    service: String,
    target: url::Url,
    tls: Tls,
    #[serde(default)]
    broker: vsmtp_config::Broker,
    #[serde(default)]
    logs: vsmtp_config::Logs,
    #[serde(skip)]
    path: std::path::PathBuf,
    extra_root_ca: Option<std::sync::Arc<TlsCertificate>>,
}

#[async_trait::async_trait]
impl DeliverySystem for Forward {
    fn routing_key(&self) -> DeliveryRoute {
        DeliveryRoute::Forward {
            service: self.service.clone(),
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
            last_deliveries: _,
            attempt: _,
        }: &CtxDelivery,
    ) -> Vec<DeliveryAttempt> {
        let message_str = mail.read().unwrap().to_string();

        assert!(self.target.scheme() == "smtp");

        let target = self.target.host_str().unwrap();
        let sni =
            <hickory_resolver::Name as std::str::FromStr>::from_str(target).unwrap_or_else(|_| {
                rcpt_to
                    .first()
                    .expect("there is always at least one recipient")
                    .forward_path
                    .domain()
            });

        vec![
            send(
                target,
                sni,
                self.target.port().unwrap_or(25),
                &hostname::get().unwrap().to_string_lossy(),
                mail_from.clone(),
                rcpt_to.clone(),
                message_str.as_bytes(),
                &self.tls,
                self.extra_root_ca.clone(),
            )
            .await,
        ]
    }
}

impl Default for Forward {
    fn default() -> Self {
        Self {
            target: url::Url::parse("smtp://localhost").unwrap(),
            service: String::default(),
            api_version: vsmtp_config::semver::VersionReq::default(),
            broker: vsmtp_config::Broker::default(),
            logs: vsmtp_config::Logs::default(),
            path: std::path::PathBuf::default(),
            tls: Tls::default(),
            extra_root_ca: None,
        }
    }
}

impl Config for Forward {
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
