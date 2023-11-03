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
use crate::{extensions::Extension, response};
use vsmtp_protocol::Domain;
use vsmtp_protocol::Reply;

// TODO; should store all the records??
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct RemoteMailExchange {
    // domain is not stored as it is the Recipients.domain
    pub mx: Domain,
    pub mx_priority: u16,
    // mx_lifetime: std::time::Instant,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct RemoteServer {
    #[dummy(faker = "IpFaker")]
    pub ip_addr: std::net::SocketAddr,
}

pub type EitherRemoteServerOrError = Result<RemoteServer, (String, RemoteServer)>;
pub type EitherGreetingsOrError = Result<Reply, (String, Reply)>;
pub type EitherEhloOrError = Result<response::Ehlo, (String, Reply)>;

/// The information stored about the remote server.
/// These information are received step by step during the delivery, so it is represented as an enum.
///
/// The discriminant of the enum describe the last action performed by the sender.
#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
#[serde(tag = "at", rename_all = "snake_case")]
pub enum RemoteInformation {
    DnsMxLookup {
        error: DnsLookupError,
    },
    /// Information received after the MX lookup.
    DnsMxIpLookup {
        mx: RemoteMailExchange,
        error: DnsLookupError,
    },
    /// Information received after the IP lookup on a Mail Exchange.
    TcpConnection {
        // wrapped in an option as there can be direct connection.
        mx: Option<RemoteMailExchange>,
        target: EitherRemoteServerOrError,
        // optional error caught while performing the next step of the exchange.
        io: Option<vsmtp_protocol::Error>,
    },
    /// Information received after a TCP connection.
    SmtpGreetings {
        mx: Option<RemoteMailExchange>,
        target: RemoteServer,
        greeting: EitherGreetingsOrError,
        io: Option<vsmtp_protocol::Error>,
    },
    /// Information received after a EHLO/HELO command.
    SmtpEhlo {
        mx: Option<RemoteMailExchange>,
        target: RemoteServer,
        greeting: Reply,
        ehlo: EitherEhloOrError,
        io: Option<vsmtp_protocol::Error>,
    },
    SmtpTlsUpgrade {
        mx: Option<RemoteMailExchange>,
        target: RemoteServer,
        greeting: Reply,
        ehlo: response::Ehlo,
        error: String,
        io: Option<vsmtp_protocol::Error>,
    },
    /// Information received after a MAIL FROM command.
    SmtpMailFrom {
        mx: Option<RemoteMailExchange>,
        target: RemoteServer,
        greeting: Reply,
        ehlo: response::Ehlo,
        mail_from: Reply,
        io: Option<vsmtp_protocol::Error>,
    },
    /// Information received after a RCPT TO command.
    SmtpRcptTo {
        mx: Option<RemoteMailExchange>,
        target: RemoteServer,
        greeting: Reply,
        ehlo: response::Ehlo,
        mail_from: Reply,
        #[dummy(faker = "(fake::Faker, 1..3)")]
        rcpt_to: Vec<Reply>,
        io: Option<vsmtp_protocol::Error>,
    },
    /// Information received after a DATA command.
    SmtpData {
        mx: Option<RemoteMailExchange>,
        target: RemoteServer,
        greeting: Reply,
        ehlo: response::Ehlo,
        mail_from: Reply,
        #[dummy(faker = "(fake::Faker, 1..3)")]
        rcpt_to: Vec<Reply>,
        data: Reply,
        io: Option<vsmtp_protocol::Error>,
    },
    /// Information received after the `\r\n.\r\n` sequence.
    SmtpDataEnd {
        mx: Option<RemoteMailExchange>,
        target: RemoteServer,
        greeting: Reply,
        ehlo: response::Ehlo,
        mail_from: Reply,
        #[dummy(faker = "(fake::Faker, 1..3)")]
        rcpt_to: Vec<Reply>,
        data: Reply,
        data_end: Reply,
        io: Option<vsmtp_protocol::Error>,
    },
}

impl RemoteInformation {
    pub fn get_status(&self, rcpt_idx: usize) -> Option<Status> {
        let reply = match self {
            Self::SmtpGreetings {
                greeting: Err((_, reply)),
                ..
            }
            | Self::SmtpEhlo {
                ehlo: Err((_, reply)),
                ..
            }
            | Self::SmtpMailFrom {
                mail_from: reply, ..
            } => reply.code(),

            Self::SmtpRcptTo { rcpt_to, .. } => match rcpt_to.get(rcpt_idx) {
                Some(rcpt_to) => rcpt_to.code(),
                None => return None,
            },

            Self::DnsMxIpLookup { .. }
            | Self::DnsMxLookup { .. }
            | Self::SmtpTlsUpgrade { .. }
            | Self::SmtpData { .. } => {
                return None;
            }
            Self::SmtpDataEnd {
                rcpt_to, data_end, ..
            } => match rcpt_to.get(rcpt_idx) {
                Some(rcpt_to) => {
                    if rcpt_to.code().value() / 100 == 2 {
                        rcpt_to.code()
                    } else {
                        data_end.code()
                    }
                }
                None => return None,
            },

            _ => todo!("{self:?}"),
        };

        Some(Status(
            reply
                .details()
                .map(ToString::to_string)
                .unwrap_or(format!("{}.0.0", reply.value() / 100)),
        ))
    }

    pub(super) fn get_action(&self, rcpt_idx: usize) -> Action {
        match self {
            Self::DnsMxLookup { .. }
            | Self::TcpConnection { .. }
            | Self::DnsMxIpLookup { .. }
            | Self::SmtpGreetings { .. }
            | Self::SmtpEhlo { .. }
            | Self::SmtpTlsUpgrade { .. }
            | Self::SmtpMailFrom { .. }
            | Self::SmtpRcptTo { .. }
            | Self::SmtpData { .. } => Action::Delayed {
                diagnostic_code: None,
                will_retry_until: None,
            },
            Self::SmtpDataEnd {
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

    pub fn save_greetings(&mut self, greeting: EitherGreetingsOrError) {
        match self {
            Self::TcpConnection {
                mx,
                target: EitherRemoteServerOrError::Ok(target),
                io: None,
            } => {
                *self = Self::SmtpGreetings {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting,
                    io: None,
                };
            }
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_ehlo(&mut self, ehlo: EitherEhloOrError) {
        match self {
            Self::SmtpGreetings {
                mx,
                target,
                greeting: EitherGreetingsOrError::Ok(greeting),
                io: None,
            } => {
                *self = Self::SmtpEhlo {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo,
                    io: None,
                };
            }
            _ => todo!("{self:?}"),
        }
    }

    const fn get_ehlo(&self) -> Option<&response::Ehlo> {
        match self {
            Self::TcpConnection { .. }
            | Self::DnsMxIpLookup { .. }
            | Self::DnsMxLookup { .. }
            | Self::SmtpGreetings { .. }
            | Self::SmtpEhlo {
                ehlo: EitherEhloOrError::Err(..),
                ..
            } => None,
            Self::SmtpEhlo {
                ehlo: EitherEhloOrError::Ok(ehlo),
                ..
            }
            | Self::SmtpTlsUpgrade { ehlo, .. }
            | Self::SmtpMailFrom { ehlo, .. }
            | Self::SmtpRcptTo { ehlo, .. }
            | Self::SmtpData { ehlo, .. }
            | Self::SmtpDataEnd { ehlo, .. } => Some(ehlo),
        }
    }

    #[must_use]
    pub fn has_extension(&self, extension: Extension) -> bool {
        self.get_ehlo().is_some_and(|r| r.contains(extension))
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn save_tls_upgrade_error(&mut self, error: std::io::Error) {
        match &self {
            Self::SmtpEhlo {
                mx,
                target,
                greeting,
                ehlo: EitherEhloOrError::Ok(ehlo),
                io: None,
            } => {
                *self = Self::SmtpTlsUpgrade {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                    error: error.to_string(),
                    io: None,
                }
            }
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_io_error(&mut self, error: vsmtp_protocol::Error) {
        match self {
            Self::TcpConnection { io, .. }
            | Self::SmtpGreetings { io, .. }
            | Self::SmtpEhlo { io, .. }
            | Self::SmtpTlsUpgrade { io, .. }
            | Self::SmtpMailFrom { io, .. }
            | Self::SmtpRcptTo { io, .. }
            | Self::SmtpData { io, .. }
            | Self::SmtpDataEnd { io, .. } => *io = Some(error),
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_mail_from(&mut self, reply: Reply) {
        match &self {
            Self::SmtpEhlo {
                mx,
                target,
                greeting,
                ehlo: EitherEhloOrError::Ok(ehlo),
                io: None,
            } => {
                *self = Self::SmtpMailFrom {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                    mail_from: reply,
                    io: None,
                }
            }
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_rcpt_to(&mut self, reply: Reply) {
        match self {
            Self::SmtpMailFrom {
                mx,
                target,
                greeting,
                ehlo,
                mail_from,
                io: None,
            } => {
                *self = Self::SmtpRcptTo {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                    mail_from: mail_from.clone(),
                    rcpt_to: vec![reply],
                    io: None,
                }
            }
            Self::SmtpRcptTo { rcpt_to, .. } => rcpt_to.push(reply),
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_data(&mut self, reply: Reply) {
        match self {
            Self::SmtpRcptTo {
                mx,
                target,
                greeting,
                ehlo,
                mail_from,
                rcpt_to,
                io: None,
            } => {
                *self = Self::SmtpData {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                    mail_from: mail_from.clone(),
                    rcpt_to: rcpt_to.clone(),
                    data: reply,
                    io: None,
                }
            }
            _ => todo!("{self:?}"),
        }
    }

    pub fn save_data_end(&mut self, reply: Reply) {
        match self {
            Self::SmtpData {
                mx,
                target,
                greeting,
                ehlo,
                mail_from,
                rcpt_to,
                data,
                io: None,
            } => {
                *self = Self::SmtpDataEnd {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: ehlo.clone(),
                    mail_from: mail_from.clone(),
                    rcpt_to: rcpt_to.clone(),
                    data: data.clone(),
                    data_end: reply,
                    io: None,
                }
            }
            _ => todo!("{self:?}"),
        }
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn finalize(&mut self) -> Self {
        match self {
            Self::SmtpDataEnd {
                mx,
                target,
                greeting,
                ehlo,
                mail_from: _,
                rcpt_to: _,
                data: _,
                data_end: _,
                io: _,
            }
            | Self::SmtpData {
                mx,
                target,
                greeting,
                ehlo,
                mail_from: _,
                rcpt_to: _,
                data: _,
                io: _,
            }
            | Self::SmtpRcptTo {
                mx,
                target,
                greeting,
                ehlo,
                mail_from: _,
                rcpt_to: _,
                io: _,
            }
            | Self::SmtpMailFrom {
                mx,
                target,
                greeting,
                ehlo,
                mail_from: _,
                io: _,
            }
            | Self::SmtpEhlo {
                mx,
                target,
                greeting,
                ehlo: EitherEhloOrError::Ok(ehlo),
                io: _,
            }
            | Self::SmtpTlsUpgrade {
                mx,
                target,
                greeting,
                ehlo,
                error: _,
                io: _,
            } => {
                let pre_transaction_value = Self::SmtpEhlo {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    ehlo: EitherEhloOrError::Ok(ehlo.clone()),
                    io: None,
                };
                std::mem::replace(self, pre_transaction_value)
            }
            Self::SmtpGreetings {
                mx,
                target,
                greeting,
                io: _,
            } => {
                let pre_transaction_value = Self::SmtpGreetings {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: greeting.clone(),
                    io: None,
                };
                std::mem::replace(self, pre_transaction_value)
            }
            Self::SmtpEhlo {
                mx,
                target,
                greeting,
                ehlo: EitherEhloOrError::Err(_),
                io: _,
            } => {
                let pre_transaction_value = Self::SmtpGreetings {
                    mx: mx.clone(),
                    target: target.clone(),
                    greeting: EitherGreetingsOrError::Ok(greeting.clone()),
                    io: None,
                };
                std::mem::replace(self, pre_transaction_value)
            }
            Self::TcpConnection { mx, target, io: _ } => {
                let pre_transaction_value = Self::TcpConnection {
                    mx: mx.clone(),
                    target: target.clone(),
                    io: None,
                };
                std::mem::replace(self, pre_transaction_value)
            }
            _ => todo!("{self:?}"),
        }
    }
}
