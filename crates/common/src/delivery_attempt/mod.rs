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

use crate::Recipient;

mod local_information;
mod remote_information;

pub use self::local_information::LocalInformation;
pub use self::remote_information::{RemoteInformation, RemoteMailExchange, RemoteServer};

pub struct Status(pub String);

// NOTE: should be implemented as a bitmask
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct ShouldNotify {
    pub on_success: bool,
    pub on_failure: bool,
    pub on_delay: bool,
    pub on_expanded: bool,
    pub on_relayed: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct DeliveryAttempt {
    // a list a rcpt which this delivery concerns,
    // NOTE: could be Vec<&>, but it is annoying to handle the lifetime
    rcpt_to: Vec<Recipient>,
    delivery_type: DeliveryType,
    pub should_notify: ShouldNotify,
}

impl DeliveryAttempt {
    #[must_use]
    pub fn new_smtp(
        rcpt_to: Vec<Recipient>,
        remote_info: RemoteInformation,
        should_notify: ShouldNotify,
    ) -> Self {
        Self {
            rcpt_to,
            delivery_type: DeliveryType::RemoteSmtp(Box::new(remote_info)),
            should_notify,
        }
    }

    #[must_use]
    pub fn new_local(
        rcpt_to: Recipient,
        local_info: LocalInformation,
        should_notify: ShouldNotify,
    ) -> Self {
        Self {
            rcpt_to: vec![rcpt_to],
            delivery_type: DeliveryType::Local(local_info),
            should_notify,
        }
    }

    #[must_use]
    pub fn get_status(&self, rcpt_idx: usize) -> Status {
        match &self.delivery_type {
            DeliveryType::Local(local) => local.into(),
            DeliveryType::RemoteSmtp(remote_information) => {
                (remote_information.as_ref(), rcpt_idx).into()
            }
        }
    }

    #[must_use]
    pub fn get_action(&self, rcpt_idx: usize) -> Action {
        match &self.delivery_type {
            DeliveryType::Local(local) => local.get_action(),
            DeliveryType::RemoteSmtp(remote_information) => remote_information.get_action(rcpt_idx),
        }
    }

    #[must_use]
    pub fn get_rcpt_index(&self, recipient: &Recipient) -> Option<usize> {
        self.rcpt_to.iter().position(|r| r == recipient)
    }

    pub fn recipients(&self) -> impl Iterator<Item = &Recipient> + '_ {
        self.rcpt_to.iter()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
enum DeliveryType {
    Local(LocalInformation),
    RemoteSmtp(Box<RemoteInformation>),
}

/// <https://www.rfc-editor.org/rfc/rfc3464#section-2.3.3>
#[derive(PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase", tag = "value")]
pub enum Action {
    Failed {
        diagnostic_code: Option<String>,
    },
    Delayed {
        diagnostic_code: Option<String>,
        will_retry_until: Option<time::OffsetDateTime>,
    },
    Delivered,
    Relayed,
    Expanded,
}

impl Action {
    #[must_use]
    pub const fn is_successful(&self) -> bool {
        matches!(self, Self::Delivered | Self::Relayed | Self::Expanded)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub enum DnsLookupError {
    NoRecord,
    Timeout,
    Other(String),
}

impl From<&DnsLookupError> for Status {
    fn from(value: &DnsLookupError) -> Self {
        match value {
            DnsLookupError::NoRecord => Self("4.4.4".to_string()),
            DnsLookupError::Timeout => Self("4.4.7".to_string()),
            DnsLookupError::Other(_) => Self("5.4.0".to_string()),
        }
    }
}

impl From<hickory_resolver::error::ResolveError> for DnsLookupError {
    fn from(value: hickory_resolver::error::ResolveError) -> Self {
        use hickory_resolver::error::ResolveErrorKind;
        match value.kind() {
            ResolveErrorKind::Message(message) => Self::Other((*message).to_string()),
            ResolveErrorKind::Msg(message) => Self::Other(message.clone()),
            ResolveErrorKind::NoRecordsFound { .. } => Self::NoRecord,
            ResolveErrorKind::Timeout => Self::Timeout,
            ResolveErrorKind::NoConnections => todo!(),
            ResolveErrorKind::Io(_) => todo!(),
            ResolveErrorKind::Proto(_) => todo!(),
            // non exhaustive
            _ => todo!(),
        }
    }
}
