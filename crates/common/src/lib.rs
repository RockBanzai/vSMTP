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
pub mod dkim;
pub mod dmarc;
pub mod dns_resolver;
pub mod extensions;
pub mod faker;
pub mod iprev;
pub mod libc;
pub mod response;
pub mod serde_helper;
pub mod spf;
pub mod stateful_ctx_received;
pub mod transfer_error;

pub use time;
pub use trust_dns_resolver;
pub use uuid;

use crate::faker::MailboxFaker;
use vsmtp_protocol::{Address, Domain, NotifyOn, OriginalRecipient};

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
