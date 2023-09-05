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
use vsmtp_delivery::{delivery_main, DeliverySystem, ShouldNotify};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Mbox {}

#[async_trait::async_trait]
impl DeliverySystem for Mbox {
    fn routing_key(&self) -> DeliveryRoute {
        DeliveryRoute::Mbox
    }

    fn get_notification_supported() -> ShouldNotify {
        ShouldNotify {
            on_success: true,
            on_failure: true,
            on_delay: true,
        }
    }

    async fn deliver(self: Arc<Self>, _: &CtxDelivery) -> Vec<DeliveryAttempt> {
        unimplemented!()
    }
}

#[derive(clap::Parser)]
#[command(author, version, about)]
struct Args {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    <Args as clap::Parser>::parse();

    let system = std::env::var("SYSTEM").expect("SYSTEM");
    let system = std::sync::Arc::from(serde_json::from_str::<Mbox>(&system)?);

    delivery_main(system).await
}
