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

use crate::{
    ctx_received::CtxReceived,
    delivery_route::DeliveryRoute,
    faker::{ClientNameFaker, DsnReturnFaker, IpFaker, NameFaker, RcptToFaker},
    tls::TlsProps,
    Mailbox, Recipient,
};
use fake::faker::time::fr_fr::DateTimeBetween;
use vsmtp_auth::{dkim::DkimVerificationResult, dmarc::Dmarc, iprev::IpRevResult, spf};
use vsmtp_mail_parser::Mail;
use vsmtp_protocol::{rustls, ClientName, Domain, DsnReturn, NotifyOn, Stage};

macro_rules! exactly {
    ($i:expr) => {
        $i..=$i
    };
}

#[derive(Debug, thiserror::Error)]
#[error("invalid state, operation valid at {expected:?}, but got {got:?}")]
pub struct StateError {
    expected: std::ops::RangeInclusive<Stage>,
    got: Stage,
}

impl StateError {
    #[must_use]
    pub fn new(expected: std::ops::RangeInclusive<Stage>, got: Stage) -> Self {
        debug_assert!(!expected.contains(&got));
        Self { expected, got }
    }
}

impl From<StateError> for Box<rhai::EvalAltResult> {
    fn from(value: StateError) -> Self {
        Self::new(value.to_string().into())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub enum StatefulCtxReceived {
    Connect {
        connect: ConnectProps,
    },
    Helo {
        connect: ConnectProps,
        helo: HeloProps,
    },
    MailFrom {
        connect: ConnectProps,
        helo: HeloProps,
        mail_from: MailFromProps,
    },
    RcptTo {
        connect: ConnectProps,
        helo: HeloProps,
        mail_from: MailFromProps,
        rcpt_to: RcptToProps,
    },
    Complete(CtxReceived),
}

impl StatefulCtxReceived {
    #[must_use]
    pub fn fake() -> Self {
        fake::Fake::fake(&fake::Faker)
    }
}

impl TryFrom<StatefulCtxReceived> for CtxReceived {
    type Error = ();

    fn try_from(value: StatefulCtxReceived) -> Result<Self, Self::Error> {
        match value {
            StatefulCtxReceived::Complete(complete) => Ok(complete),
            _ => Err(()),
        }
    }
}

impl StatefulCtxReceived {
    #[must_use]
    pub const fn get_stage(&self) -> Stage {
        match self {
            Self::Connect { .. } => Stage::Connect,
            Self::Helo { .. } => Stage::Helo,
            Self::MailFrom { .. } => Stage::MailFrom,
            Self::RcptTo { .. } => Stage::RcptTo,
            Self::Complete { .. } => Stage::Finished,
        }
    }

    #[must_use]
    pub const fn new(props: ConnectProps) -> Self {
        Self::Connect { connect: props }
    }

    #[must_use]
    pub const fn server_name(&self) -> &Domain {
        &self.get_connect().server_name
    }

    #[must_use]
    pub fn produce_new(&self) -> Self {
        let mut new_instance = self.clone();
        new_instance.reset();
        new_instance
    }

    pub fn set_helo(
        &mut self,
        client_name: ClientName,
        using_deprecated: bool,
    ) -> Result<&mut Self, StateError> {
        match self {
            Self::Connect { connect } | Self::Helo { connect, .. } => {
                *self = Self::Helo {
                    connect: connect.clone(),
                    helo: HeloProps {
                        client_name,
                        using_deprecated,
                        spf_helo_identity: None,
                    },
                };
                Ok(self)
            }
            _ => Err(StateError::new(
                Stage::Connect..=Stage::Helo,
                self.get_stage(),
            )),
        }
    }

    /// Has the connection been encrypted using TLS ?
    #[inline]
    #[must_use]
    pub const fn is_secured(&self) -> bool {
        match self {
            Self::Connect { connect }
            | Self::Helo { connect, .. }
            | Self::MailFrom { connect, .. }
            | Self::RcptTo { connect, .. }
            | Self::Complete(CtxReceived { connect, .. }) => connect.tls.is_some(),
        }
    }

    /// Change a raw transaction into a secured one by setting the [`TlsProps`].
    ///
    /// # Errors
    ///
    /// * state if not [`StatefulCtxReceived::Connect`] or [`StatefulCtxReceived::Helo`]
    #[inline]
    pub fn set_secured(
        &mut self,
        sni: Option<Domain>,
        protocol_version: rustls::ProtocolVersion,
        cipher_suite: rustls::CipherSuite,
        peer_certificates: Option<Vec<rustls::Certificate>>,
        alpn_protocol: Option<Vec<u8>>,
    ) -> Result<(), StateError> {
        match self {
            Self::Connect { connect, .. } | Self::Helo { connect, .. } => {
                connect.tls = Some(crate::tls::TlsProps {
                    protocol_version: crate::tls::ProtocolVersion(protocol_version),
                    cipher_suite: crate::tls::CipherSuite(cipher_suite),
                    peer_certificates,
                    alpn_protocol,
                });

                if let Some(sni) = sni {
                    connect.server_name = sni;
                }

                Ok(())
            }
            Self::MailFrom { .. } | Self::RcptTo { .. } | Self::Complete(_) => Err(
                StateError::new(Stage::Connect..=Stage::Helo, self.get_stage()),
            ),
        }
    }

    pub fn set_mail_from(
        &mut self,
        reverse_path: Option<Mailbox>,
        envelop_id: Option<String>,
        ret: Option<DsnReturn>,
    ) -> Result<&mut Self, StateError> {
        match self {
            Self::Helo { connect, helo } => {
                *self = Self::MailFrom {
                    connect: connect.clone(),
                    helo: helo.clone(),
                    mail_from: MailFromProps {
                        reverse_path,
                        mail_timestamp: time::OffsetDateTime::now_utc(),
                        message_uuid: uuid::Uuid::new_v4(),
                        envelop_id,
                        ret,
                        spf_mail_from_identity: None,
                    },
                };
                Ok(self)
            }
            _ => Err(StateError::new(exactly!(Stage::Helo), self.get_stage())),
        }
    }

    pub fn set_rcpt_to(
        &mut self,
        route: DeliveryRoute,
        rcpt: Recipient,
    ) -> Result<&mut Self, StateError> {
        match self {
            Self::MailFrom {
                connect,
                helo,
                mail_from,
            } => {
                *self = Self::RcptTo {
                    connect: connect.clone(),
                    helo: helo.clone(),
                    mail_from: mail_from.clone(),
                    rcpt_to: RcptToProps {
                        recipient: std::iter::once((route, vec![rcpt])).collect(),
                    },
                };
                Ok(self)
            }
            Self::RcptTo {
                connect: _,
                helo: _,
                mail_from: _,
                rcpt_to,
            } => {
                if let Some(values) = rcpt_to.recipient.get_mut(&route) {
                    values.push(rcpt);
                } else {
                    rcpt_to.recipient.insert(route, vec![rcpt]);
                }
                Ok(self)
            }
            _ => Err(StateError::new(
                Stage::MailFrom..=Stage::RcptTo,
                self.get_stage(),
            )),
        }
    }

    pub fn set_complete(&mut self, mail: Mail) -> Result<&mut Self, StateError> {
        match self {
            Self::RcptTo {
                connect,
                helo,
                mail_from,
                rcpt_to,
            } => {
                *self = Self::Complete(CtxReceived {
                    connect: connect.clone(),
                    helo: helo.clone(),
                    mail_from: mail_from.clone(),
                    rcpt_to: RcptToProps {
                        recipient: rcpt_to
                            .recipient
                            .iter()
                            .filter(|(_, v)| !v.is_empty())
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    },
                    mail: std::sync::Arc::new(std::sync::RwLock::new(mail)),
                    complete: CompleteProps {
                        dkim: None,
                        dmarc: None,
                    },
                });
                Ok(self)
            }
            _ => Err(StateError::new(exactly!(Stage::RcptTo), self.get_stage())),
        }
    }

    pub fn reset(&mut self) {
        let connect = self.get_connect().clone();
        if let Some(helo) = self.get_helo().ok().cloned() {
            *self = Self::Helo { connect, helo };
        } else {
            *self = Self::Connect { connect };
        }
    }

    #[must_use]
    pub const fn get_connect(&self) -> &ConnectProps {
        match self {
            Self::Connect { connect }
            | Self::Helo { connect, .. }
            | Self::MailFrom { connect, .. }
            | Self::RcptTo { connect, .. }
            | Self::Complete(CtxReceived { connect, .. }) => connect,
        }
    }

    pub fn mut_connect(&mut self) -> &mut ConnectProps {
        match self {
            Self::Connect { connect }
            | Self::Helo { connect, .. }
            | Self::MailFrom { connect, .. }
            | Self::RcptTo { connect, .. }
            | Self::Complete(CtxReceived { connect, .. }) => connect,
        }
    }

    pub fn get_helo(&self) -> Result<&HeloProps, StateError> {
        match self {
            Self::Helo { helo, .. }
            | Self::MailFrom { helo, .. }
            | Self::RcptTo { helo, .. }
            | Self::Complete(CtxReceived { helo, .. }) => Ok(helo),
            Self::Connect { .. } => Err(StateError::new(
                Stage::Helo..=Stage::Finished,
                self.get_stage(),
            )),
        }
    }

    pub fn mut_helo(&mut self) -> Result<&mut HeloProps, StateError> {
        match self {
            Self::Helo { helo, .. }
            | Self::MailFrom { helo, .. }
            | Self::RcptTo { helo, .. }
            | Self::Complete(CtxReceived { helo, .. }) => Ok(helo),
            Self::Connect { .. } => Err(StateError::new(
                Stage::Helo..=Stage::Finished,
                self.get_stage(),
            )),
        }
    }

    pub fn get_mail_from(&self) -> Result<&MailFromProps, StateError> {
        match self {
            Self::MailFrom { mail_from, .. }
            | Self::RcptTo { mail_from, .. }
            | Self::Complete(CtxReceived { mail_from, .. }) => Ok(mail_from),
            _ => Err(StateError::new(
                Stage::MailFrom..=Stage::Finished,
                self.get_stage(),
            )),
        }
    }

    pub fn mut_mail_from(&mut self) -> Result<&mut MailFromProps, StateError> {
        match self {
            Self::MailFrom { mail_from, .. }
            | Self::RcptTo { mail_from, .. }
            | Self::Complete(CtxReceived { mail_from, .. }) => Ok(mail_from),
            _ => Err(StateError::new(
                Stage::MailFrom..=Stage::Finished,
                self.get_stage(),
            )),
        }
    }

    pub fn get_rcpt_to(&self) -> Result<&RcptToProps, StateError> {
        match self {
            Self::RcptTo { rcpt_to, .. } | Self::Complete(CtxReceived { rcpt_to, .. }) => {
                Ok(rcpt_to)
            }
            _ => Err(StateError::new(
                Stage::RcptTo..=Stage::Finished,
                self.get_stage(),
            )),
        }
    }

    pub fn mut_rcpt_to(&mut self) -> Result<&mut RcptToProps, StateError> {
        match self {
            Self::RcptTo { rcpt_to, .. } | Self::Complete(CtxReceived { rcpt_to, .. }) => {
                Ok(rcpt_to)
            }
            _ => Err(StateError::new(
                Stage::RcptTo..=Stage::Finished,
                self.get_stage(),
            )),
        }
    }

    pub fn get_mail<O>(&self, f: impl FnOnce(&Mail) -> O) -> Result<O, StateError> {
        match self {
            Self::Complete(CtxReceived { mail, .. }) => Ok(f(&mail.read().unwrap())),
            _ => Err(StateError::new(exactly!(Stage::Finished), self.get_stage())),
        }
    }

    pub fn get_mail_arc(&self) -> Result<std::sync::Arc<std::sync::RwLock<Mail>>, StateError> {
        match self {
            Self::Complete(CtxReceived { mail, .. }) => Ok(mail.clone()),
            _ => Err(StateError::new(exactly!(Stage::Finished), self.get_stage())),
        }
    }

    pub fn mut_mail<O>(&mut self, f: impl FnOnce(&mut Mail) -> O) -> Result<O, StateError> {
        match self {
            Self::Complete(CtxReceived { mail, .. }) => Ok(f(&mut mail.write().unwrap())),
            _ => Err(StateError::new(exactly!(Stage::Finished), self.get_stage())),
        }
    }

    pub fn mut_complete(&mut self) -> Result<&mut CompleteProps, StateError> {
        match self {
            Self::Complete(CtxReceived { complete, .. }) => Ok(complete),
            _ => Err(StateError::new(exactly!(Stage::Finished), self.get_stage())),
        }
    }

    pub fn get_complete(&self) -> Result<&CompleteProps, StateError> {
        match self {
            Self::Complete(CtxReceived { complete, .. }) => Ok(complete),
            _ => Err(StateError::new(exactly!(Stage::Finished), self.get_stage())),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct SaslAuthProps {
    pub cancel_count: usize,
    pub is_authenticated: bool,
    #[dummy(faker = "MechanismFaker")]
    pub mechanism: vsmtp_protocol::auth::Mechanism,
    #[dummy(faker = "CredentialsFaker")]
    pub credentials: vsmtp_protocol::auth::Credentials,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct ConnectProps {
    #[serde(with = "time::serde::iso8601")]
    pub connect_timestamp: time::OffsetDateTime,
    pub connect_uuid: uuid::Uuid,
    #[dummy(faker = "IpFaker")]
    pub client_addr: std::net::SocketAddr,
    #[dummy(faker = "IpFaker")]
    pub server_addr: std::net::SocketAddr,
    #[dummy(faker = "NameFaker")]
    pub server_name: Domain,
    pub sasl: Option<SaslAuthProps>,
    pub iprev: Option<IpRevResult>,
    /// This field is `Some` when the client and server
    /// exchange data through a secure tunnel.
    pub tls: Option<TlsProps>,
}

struct CredentialsFaker;
impl fake::Dummy<CredentialsFaker> for vsmtp_protocol::auth::Credentials {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &CredentialsFaker, _: &mut R) -> Self {
        todo!()
    }
}

struct MechanismFaker;
impl fake::Dummy<MechanismFaker> for vsmtp_protocol::auth::Mechanism {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &MechanismFaker, _: &mut R) -> Self {
        todo!()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct HeloProps {
    #[dummy(faker = "ClientNameFaker")]
    pub client_name: ClientName,
    pub using_deprecated: bool,
    pub spf_helo_identity: Option<std::sync::Arc<spf::Result>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct MailFromProps {
    // TODO:
    // * rfc 4954 : "AUTH="
    // * rfc 1870 : "SIZE="
    // * rfc 6152 : "BODY="
    // * rfc 6531 : "SMTPUTF8"
    // * rfc 3885 : "MTRK"
    // * rfc 4865 : "FUTURERELEASE"
    // pub spf: Option<spf::Result>,
    // ...
    pub reverse_path: Option<Mailbox>,
    #[serde(with = "time::serde::iso8601")]
    #[dummy(faker = "DateTimeBetween(
        time::macros::datetime!(2000-01-01 0:00 UTC),
        time::OffsetDateTime::now_utc()
    )")]
    pub mail_timestamp: time::OffsetDateTime,
    pub message_uuid: uuid::Uuid,
    pub envelop_id: Option<String>,
    pub spf_mail_from_identity: Option<std::sync::Arc<spf::Result>>,
    #[dummy(faker = "DsnReturnFaker")]
    pub ret: Option<DsnReturn>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct RcptToProps {
    #[dummy(faker = "RcptToFaker")]
    pub recipient: std::collections::HashMap<DeliveryRoute, Vec<Recipient>>,
}

impl RcptToProps {
    /// Get an iterator over the recipients without the delivery route.
    pub fn recipient_values(&self) -> impl Iterator<Item = &Recipient> {
        self.recipient.values().flatten()
    }

    /// Get a mutable iterator over the recipients without the delivery route.
    pub fn recipient_values_mut(&mut self) -> impl Iterator<Item = &mut Recipient> {
        self.recipient.values_mut().flatten()
    }

    /// Add a recipient to the list with a specific delivery route.
    pub fn add_recipient_with_route(&mut self, new_recipient: Mailbox, route: DeliveryRoute) {
        let new_recipient = Recipient {
            forward_path: new_recipient,
            original_forward_path: None,
            notify_on: NotifyOn::Some {
                success: false,
                failure: true,
                delay: false,
            },
        };

        // PERF: prevent the recipient clone due to the entry API.
        self.recipient
            .entry(route)
            .and_modify(|recipients| recipients.push(new_recipient.clone()))
            .or_insert_with(|| vec![new_recipient]);
    }

    /// Remove a single recipient from the list.
    pub fn remove_recipient(&mut self, addr: &Mailbox) {
        self.recipient.values_mut().for_each(|v| {
            v.retain(|r| r.forward_path != *addr);
        });
    }

    /// Replace a recipient address by another. The notification settings stay unchanged.
    pub fn rewrite_recipient(&mut self, old_addr: &Mailbox, new_addr: Mailbox) {
        if let Some(r) = self
            .recipient_values_mut()
            .find(|r| r.forward_path == *old_addr)
        {
            r.forward_path = new_addr;
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct CompleteProps {
    pub dkim: Option<std::sync::Arc<Vec<DkimVerificationResult>>>,
    pub dmarc: Option<std::sync::Arc<Dmarc>>,
}
