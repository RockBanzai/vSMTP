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

use super::{Action, DnsLookupError, Status};
use crate::faker::IpFaker;
use crate::faker::NameFaker;
use crate::faker::ReplyFaker;
use crate::transfer_error::Delivery;
use crate::{extensions::Extension, response};
use vsmtp_protocol::Domain;
use vsmtp_protocol::Reply;

// TODO; should store all the records??
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct RemoteMailExchange {
    // domain is not stored as it is the Recipients.domain
    #[dummy(faker = "NameFaker")]
    pub mx: Domain,
    pub mx_priority: u16,
    // mx_lifetime: std::time::Instant,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct RemoteServer {
    #[dummy(faker = "IpFaker")]
    pub ip_addr: std::net::SocketAddr,
}

/// The information that is stored about the remote server.
/// These information are received step by step during the delivery, so it is represented as an enum.
#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub enum RemoteInformation {
    ConnectError {
        error: String,
    },
    StartTlsError {
        error: Delivery,
    },
    TlsError {
        error: Delivery,
    },
    MxLookupError {
        error: DnsLookupError,
    },
    /// Information received after the MX lookup.
    MxLookup {
        mx: RemoteMailExchange,
        error: DnsLookupError,
    },
    /// Information received after the IP lookup on a Mail Exchange.
    IpLookup {
        mx: RemoteMailExchange,
        target: RemoteServer,
    },
    /// Information received after a TCP connection.
    Greetings {
        mx: RemoteMailExchange,
        target: RemoteServer,
        #[dummy(faker = "ReplyFaker")]
        greeting: Reply,
    },
    /// Information received after a EHLO/HELO command.
    Ehlo {
        mx: RemoteMailExchange,
        target: RemoteServer,
        #[dummy(faker = "ReplyFaker")]
        greeting: Reply,
        ehlo: response::Ehlo,
    },
    /// Information received after a MAIL FROM command.
    MailFrom {
        mx: RemoteMailExchange,
        target: RemoteServer,
        #[dummy(faker = "ReplyFaker")]
        greeting: Reply,
        ehlo: response::Ehlo,
        #[dummy(faker = "ReplyFaker")]
        mail_from: Reply,
    },
    /// Information received after a RCPT TO command.
    RcptTo {
        mx: RemoteMailExchange,
        target: RemoteServer,
        #[dummy(faker = "ReplyFaker")]
        greeting: Reply,
        ehlo: response::Ehlo,
        #[dummy(faker = "ReplyFaker")]
        mail_from: Reply,
        #[dummy(faker = "(ReplyFaker, 1..3)")]
        rcpt_to: Vec<Reply>,
    },
    /// Information received after a DATA command.
    Data {
        mx: RemoteMailExchange,
        target: RemoteServer,
        #[dummy(faker = "ReplyFaker")]
        greeting: Reply,
        ehlo: response::Ehlo,
        #[dummy(faker = "ReplyFaker")]
        mail_from: Reply,
        #[dummy(faker = "(ReplyFaker, 1..3)")]
        rcpt_to: Vec<Reply>,
        #[dummy(faker = "ReplyFaker")]
        data: Reply,
    },
    /// Information received after the `\r\n.\r\n` sequence.
    DataEnd {
        mx: RemoteMailExchange,
        target: RemoteServer,
        #[dummy(faker = "ReplyFaker")]
        greeting: Reply,
        ehlo: response::Ehlo,
        #[dummy(faker = "ReplyFaker")]
        mail_from: Reply,
        #[dummy(faker = "(ReplyFaker, 1..3)")]
        rcpt_to: Vec<Reply>,
        #[dummy(faker = "ReplyFaker")]
        data: Reply,
        #[dummy(faker = "ReplyFaker")]
        data_end: Reply,
    },
}

impl From<(&RemoteInformation, usize)> for Status {
    fn from((value, idx): (&RemoteInformation, usize)) -> Self {
        match value {
            RemoteInformation::ConnectError { .. } => todo!(),
            RemoteInformation::TlsError { .. } => todo!(),
            RemoteInformation::StartTlsError { .. } => todo!(),
            RemoteInformation::MxLookup { mx: _, error }
            | RemoteInformation::MxLookupError { error } => error.into(),
            RemoteInformation::IpLookup { .. } => todo!(),
            RemoteInformation::Greetings { .. } => todo!(),
            RemoteInformation::Ehlo { .. } => todo!(),
            RemoteInformation::MailFrom { .. } => todo!(),
            RemoteInformation::RcptTo { .. } => todo!(),
            RemoteInformation::Data { .. } => todo!(),
            RemoteInformation::DataEnd {
                rcpt_to, data_end, ..
            } => {
                let rcpt_code = rcpt_to.get(idx).unwrap().code();
                if rcpt_code.value() / 100 == 5 {
                    return Self(rcpt_code.details().unwrap_or("5.0.0").to_string());
                } else if rcpt_code.value() / 100 == 4 {
                    return Self(rcpt_code.details().unwrap_or("4.0.0").to_string());
                }

                Self(data_end.code().details().map_or_else(
                    || format!("{}.0.0", data_end.code().value()),
                    ToString::to_string,
                ))
            }
        }
    }
}

impl RemoteInformation {
    pub(super) fn get_action(&self, rcpt_idx: usize) -> Action {
        match self {
            Self::MxLookupError { .. }
            | Self::ConnectError { .. }
            | Self::TlsError { .. }
            | Self::MxLookup { .. }
            | Self::IpLookup { .. }
            | Self::Greetings { .. }
            | Self::Ehlo { .. }
            | Self::MailFrom { .. }
            | Self::RcptTo { .. }
            | Self::Data { .. } => Action::Delayed {
                diagnostic_code: None,
                will_retry_until: None,
            },
            // No need to retry, the server does not support starttls and it is required.
            // FIXME: should have a "do not retry" action.
            Self::StartTlsError { error } => Action::Failed {
                diagnostic_code: Some(error.to_string()),
            },
            Self::DataEnd {
                rcpt_to, data_end, ..
            } => {
                let rcpt_code = rcpt_to.get(rcpt_idx).unwrap().code();
                if rcpt_code.value() / 100 == 5 {
                    return Action::Failed {
                        diagnostic_code: None,
                    };
                } else if rcpt_code.value() / 100 == 4 {
                    return Action::Delayed {
                        diagnostic_code: None,
                        will_retry_until: None,
                    };
                }

                // NOTE: the other reply of the transaction are not checked, because a non 2xx reply code
                // would have been handled elsewhere.
                match data_end.code().value() / 100 {
                    2 => Action::Delivered,
                    4 => Action::Delayed {
                        diagnostic_code: None,
                        will_retry_until: None,
                    },
                    5 => Action::Failed {
                        diagnostic_code: None,
                    },
                    _ => unreachable!(),
                }
            }
        }
    }

    pub fn save_greetings(&mut self, reply: Reply) {
        match self {
            Self::IpLookup { mx, target } => {
                *self = Self::Greetings {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: reply,
                };
            }
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_ehlo(&mut self, reply: response::Ehlo) {
        match self {
            Self::Greetings {
                mx,
                target,
                greeting,
            } => {
                *self = Self::Ehlo {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: reply,
                };
            }
            _ => todo!("{self:?}"),
        }
    }

    const fn get_ehlo(&self) -> Option<&response::Ehlo> {
        match self {
            Self::IpLookup { .. }
            | Self::ConnectError { .. }
            | Self::StartTlsError { .. }
            | Self::TlsError { .. }
            | Self::MxLookup { .. }
            | Self::MxLookupError { .. }
            | Self::Greetings { .. } => None,
            Self::Ehlo { ehlo, .. }
            | Self::MailFrom { ehlo, .. }
            | Self::RcptTo { ehlo, .. }
            | Self::Data { ehlo, .. }
            | Self::DataEnd { ehlo, .. } => Some(ehlo),
        }
    }

    #[must_use]
    pub fn has_extension(&self, extension: Extension) -> bool {
        self.get_ehlo().is_some_and(|r| r.contains(&extension))
    }

    pub fn save_mail_from(&mut self, reply: Reply) {
        match &self {
            Self::Ehlo {
                mx,
                target,
                greeting,
                ehlo,
            } => {
                *self = Self::MailFrom {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                    mail_from: reply,
                }
            }
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_rcpt_to(&mut self, reply: Reply) {
        match self {
            Self::MailFrom {
                mx,
                target,
                greeting,
                ehlo,
                mail_from,
            } => {
                *self = Self::RcptTo {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                    mail_from: mail_from.clone(),
                    rcpt_to: vec![reply],
                }
            }
            Self::RcptTo { rcpt_to, .. } => rcpt_to.push(reply),
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_data(&mut self, reply: Reply) {
        match self {
            Self::RcptTo {
                mx,
                target,
                greeting,
                ehlo,
                mail_from,
                rcpt_to,
            } => {
                *self = Self::Data {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                    mail_from: mail_from.clone(),
                    rcpt_to: rcpt_to.clone(),
                    data: reply,
                }
            }
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_data_end(&mut self, reply: Reply) {
        match self {
            Self::Data {
                mx,
                target,
                greeting,
                ehlo,
                mail_from,
                rcpt_to,
                data,
            } => {
                *self = Self::DataEnd {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                    mail_from: mail_from.clone(),
                    rcpt_to: rcpt_to.clone(),
                    data: data.clone(),
                    data_end: reply,
                }
            }
            _ => todo!("{self:?}"),
        }
    }

    #[must_use]
    pub fn finalize(&mut self) -> Self {
        match self {
            Self::DataEnd {
                mx,
                target,
                greeting,
                ehlo,
                mail_from: _,
                rcpt_to: _,
                data: _,
                data_end: _,
            } => {
                let pre_transaction_value = Self::Ehlo {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                };
                std::mem::replace(self, pre_transaction_value)
            }
            _ => todo!("{self:?}"),
        }
    }
}
