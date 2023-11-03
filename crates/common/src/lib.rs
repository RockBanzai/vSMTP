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

pub mod api;
pub mod broker;
pub mod ctx_delivery;
pub mod ctx_received;
pub mod delivery_attempt;
pub mod delivery_route;
pub mod dns_resolver;
pub mod extensions;
pub mod faker;
pub mod libc;
pub mod response;
pub mod stateful_ctx_received;
pub mod tls;

pub use hickory_resolver;
pub use time;
pub use uuid;

use crate::faker::MailboxFaker;
use vsmtp_protocol::{Address, Domain, NotifyOn, OriginalRecipient};

pub async fn init_logs(
    conn: &lapin::Connection,
    config: &vsmtp_config::Logs,
    service_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::prelude::*;
    let filter = tracing_subscriber::filter::Targets::new()
        .with_targets(config.levels.clone())
        .with_default(config.default_level);

    let (layer, dispatcher) = tracing_amqp::layer(conn, service_name).await;

    tracing_subscriber::registry()
        .with(layer.with_filter(filter))
        .try_init()
        .unwrap();
    tokio::spawn(dispatcher);

    std::panic::set_hook(Box::new(|e| {
        // TODO: check a way to improve formatting for this.
        tracing::error!(?e, "panic occurred");
    }));

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct Mailbox(#[dummy(faker = "MailboxFaker { domain: None }")] pub Address);

impl std::fmt::Display for Mailbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Mailbox {
    #[must_use]
    pub fn local_part(&self) -> &str {
        self.0.local_part()
    }

    #[must_use]
    pub fn domain(&self) -> Domain {
        self.0.domain()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    pub forward_path: Mailbox,
    /// rfc 3461
    pub original_forward_path: Option<OriginalRecipient>,
    /// rfc 3461
    pub notify_on: NotifyOn,
}

// TODO: enhance that
#[derive(Debug, thiserror::Error)]
pub enum DeserializeError {
    #[error("deserialize error: {0}")]
    Error(serde_json::Error),
}

// TODO: enhance that
// NOTE: do we really want to handle serialization error ?
// our model are supposed to be valid and tested, so .unwrap() is acceptable ?
#[derive(Debug, thiserror::Error)]
pub enum SerializeError {
    #[error("serialize error: {0}")]
    Error(serde_json::Error),
}
