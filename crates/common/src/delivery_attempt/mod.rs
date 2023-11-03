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

use crate::{Mailbox, Recipient};

mod local_information;
mod remote_information;

pub use local_information::LocalInformation;
pub use remote_information::{
    EitherEhloOrError, EitherGreetingsOrError, EitherRemoteServerOrError, RemoteInformation,
    RemoteMailExchange, RemoteServer,
};

pub struct Status(pub String);

bitflags::bitflags! {
    #[derive(Debug, PartialEq, Eq, Copy, Clone, serde::Serialize, serde::Deserialize,)]
    #[serde(transparent)]
    pub struct ShouldNotify: u32 {
        const Success  = 1 << 0;
        const Failure  = 1 << 1;
        const Delay    = 1 << 2;
        const Expanded = 1 << 3;
        const Relayed  = 1 << 4;
    }
}

struct ShouldNotifyFaker;
impl fake::Dummy<ShouldNotifyFaker> for ShouldNotify {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &ShouldNotifyFaker, rng: &mut R) -> Self {
        let mut should_notify = Self::empty();
        if rng.gen_bool(0.5) {
            should_notify |= Self::Success;
        }
        if rng.gen_bool(0.5) {
            should_notify |= Self::Failure;
        }
        if rng.gen_bool(0.5) {
            should_notify |= Self::Delay;
        }
        if rng.gen_bool(0.5) {
            should_notify |= Self::Expanded;
        }
        if rng.gen_bool(0.5) {
            should_notify |= Self::Relayed;
        }
        should_notify
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct DeliveryAttempt {
    // the list of recipient concerned by this attempt
    // NOTE: could be Vec<&>, but it is annoying to handle the lifetime
    recipients: Vec<Mailbox>,
    #[dummy(faker = "ShouldNotifyFaker")]
    should_notify: ShouldNotify,
    inner: DeliveryType,
}

impl DeliveryAttempt {
    #[must_use]
    pub fn new_remote(
        rcpt_to: Vec<Mailbox>,
        remote_info: RemoteInformation,
        should_notify: ShouldNotify,
    ) -> Self {
        Self {
            recipients: rcpt_to,
            inner: DeliveryType::RemoteSmtp(Box::new(remote_info)),
            should_notify,
        }
    }

    #[must_use]
    pub fn new_local(
        rcpt_to: Mailbox,
        local_info: LocalInformation,
        should_notify: ShouldNotify,
    ) -> Self {
        Self {
            recipients: vec![rcpt_to],
            inner: DeliveryType::Local(local_info),
            should_notify,
        }
    }

    #[must_use]
    pub const fn should_notify_on(&self, on: ShouldNotify) -> bool {
        self.should_notify.contains(on)
    }

    #[must_use]
    pub fn get_status(&self, rcpt_idx: usize) -> Status {
        match &self.inner {
            DeliveryType::Local(local) => local.into(),
            DeliveryType::RemoteSmtp(remote_information) => {
                remote_information.as_ref().get_status(rcpt_idx).unwrap()
            }
        }
    }

    #[must_use]
    pub fn get_action(&self, rcpt_idx: usize) -> Action {
        match &self.inner {
            DeliveryType::Local(local) => local.get_action(),
            DeliveryType::RemoteSmtp(remote_information) => remote_information.get_action(rcpt_idx),
        }
    }

    #[must_use]
    pub fn get_rcpt_index(&self, recipient: &Recipient) -> Option<usize> {
        self.recipients
            .iter()
            .position(|r| r.0 == recipient.forward_path.0)
    }

    pub fn recipients(&self) -> impl Iterator<Item = &Mailbox> + '_ {
        self.recipients.iter()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
#[serde(tag = "type")]
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
