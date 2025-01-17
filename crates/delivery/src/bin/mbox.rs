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
use vsmtp_delivery::{delivery_main, DeliverySystem};

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Mbox {
    #[serde(skip, default = "default_mbox_hostname")]
    name: String,
}

fn default_mbox_hostname() -> String {
    "mbox".to_string()
}

#[async_trait::async_trait]
impl DeliverySystem for Mbox {
    fn name(&self) -> &str {
        &self.name
    }

    fn routing_key(&self) -> DeliveryRoute {
        DeliveryRoute::Mbox
    }

    async fn deliver(self: Arc<Self>, _: &CtxDelivery) -> Vec<DeliveryAttempt> {
        unimplemented!()
    }
}

impl Config for Mbox {
    fn api_version(&self) -> &vsmtp_config::semver::VersionReq {
        todo!()
    }

    fn broker(&self) -> &vsmtp_config::broker::Broker {
        todo!()
    }

    fn logs(&self) -> &vsmtp_config::logs::Logs {
        todo!()
    }

    fn path(&self) -> &std::path::Path {
        todo!()
    }
}

#[derive(clap::Parser)]
#[command(author, version, about)]
struct Args {
    /// Path to the rhai configuration file.
    #[arg(short, long, default_value_t = String::from("/etc/vsmtp/mbox/conf.d/config.rhai"))]
    pub config: String,
}

#[tokio::main]
async fn main() {
    let Args { config } = <Args as clap::Parser>::parse();

    let system = match Mbox::from_rhai_file(&config) {
        Ok(cfg) => std::sync::Arc::new(cfg),
        Err(error) => {
            eprintln!("Failed to initialize mbox delivery configuration: {error}");
            return;
        }
    };

    if let Err(error) = delivery_main(system).await {
        tracing::error!("Failed to run mbox delivery: {error}");
    }
}
