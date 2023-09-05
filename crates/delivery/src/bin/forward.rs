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
use vsmtp_delivery::{delivery_main, smtp::send, DeliverySystem, ShouldNotify};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Forward {
    service: String,
    target: url::Url,
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

#[derive(clap::Parser)]
#[command(author, version, about)]
struct Args {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    <Args as clap::Parser>::parse();

    let system = std::env::var("SYSTEM").expect("SYSTEM");
    let system = std::sync::Arc::from(serde_json::from_str::<Forward>(&system)?);

    delivery_main(system).await
}
