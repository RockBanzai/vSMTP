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

use crate::Domain;
use vsmtp_protocol::ReplyCode;

/// The envelop to use for the SMTP exchange is invalid
#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "testing", derive(PartialEq, Eq))]
pub enum Envelop {
    /// No rcpt provided, therefor no `RCPT TO` can be sent to the remote server
    #[error("the envelop does not contain any recipient")]
    NoRecipient,
    // TODO: add too many rcpt
}

/// Error produced by local delivery method (Maildir / Mbox)
#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "testing", derive(PartialEq, Eq))]
pub enum LocalDelivery {
    /// The requested mailbox does not exist on the system
    #[error("mailbox `{mailbox}` does not exist")]
    MailboxDoNotExist {
        /// Mailbox name
        // FIXME: should be a type `Mailbox` ?
        mailbox: String,
    },
    ///
    // FIXME: should be std::io::Error ?
    #[error("todo")]
    Other(String),
}

/// Error produced by the ip/mx lookup of a target
#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "testing", derive(PartialEq, Eq))]
pub enum Lookup {
    /// No records found for the given query
    #[error("record not found")]
    NoRecords {},

    /// The lookup returned a record with a null MX
    #[error("null MX record found for '{domain}'")]
    ContainsNullMX {
        /// Domain of the DNS zone
        domain: Domain,
    },

    /// The lookup timed out
    #[error("timed out")]
    TimedOut,

    ///
    #[error("no connections available")]
    NoConnections,

    ///
    // FIXME: should handle all the IO case ..?
    #[error("io error: {0}")]
    IO(String),

    ///
    // FIXME: should handle all the proto case ..?
    #[error("dns-proto error: {0}")]
    Proto(String),

    ///
    #[error("message: {0}")]
    Message(String),

    ///
    #[error("not implemented")]
    NotImplemented,
}

/// Error produced by the queue manager
#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "testing", derive(PartialEq, Eq))]
pub enum Queuer {
    /// The recipient is still in waiting status after a delivery attempt.
    #[error("recipient is still in status waiting")]
    StillWaiting,

    /// Failed too many time to deliver the email.
    #[error("max deferred attempt reached")]
    MaxDeferredAttemptReached,
}

/// Errors produced by a SMTP exchange
#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "testing", derive(PartialEq, Eq))]
pub enum Delivery {
    /// Failed to parse the reply of the server
    #[error("failed to parse the reply of the server: source={}",
        with_source
            .as_ref()
            .map_or("null", String::as_str)
    )]
    ReplyParsing {
        /// The source of the error
        with_source: Option<String>,
    },

    /// The server replied with a permanent error `5xx`
    #[error("permanent error: {reply}: {}",
        with_source
            .as_ref()
            .map_or("null", String::as_str)
    )]
    Permanent {
        /// The reply code
        reply: ReplyCode,
        /// The source of the error
        with_source: Option<String>,
    },

    /// The server replied with a transient error `4xx`
    #[error("transient error: {reply}: {}",
        with_source
            .as_ref()
            .map_or("null", String::as_str)
    )]
    Transient {
        /// The reply code
        reply: ReplyCode,
        /// The source of the error
        with_source: Option<String>,
    },

    /// Error caused by the TLS
    #[error("tls: {}",
        with_source
            .as_ref()
            .map_or("null", String::as_str)
    )]
    Tls {
        /// The source of the error
        with_source: Option<String>,
    },

    /// Internal error of the client
    #[error("client: {}",
        with_source
            .as_ref()
            .map_or("null", String::as_str)
    )]
    Client {
        /// The source of the error
        with_source: Option<String>,
    },

    /// Error due to the underlying connection
    #[error("connection: {}",
        with_source
            .as_ref()
            .map_or("null", String::as_str)
    )]
    Connection {
        /// The source of the error
        with_source: Option<String>,
    },
}

impl From<std::io::Error> for Delivery {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        Self::Connection {
            with_source: Some(err.to_string()),
        }
    }
}

impl From<trust_dns_resolver::error::ResolveError> for Lookup {
    #[inline]
    fn from(error: trust_dns_resolver::error::ResolveError) -> Self {
        match error.kind() {
            trust_dns_resolver::error::ResolveErrorKind::Message(e) => {
                Self::Message((*e).to_owned())
            }
            trust_dns_resolver::error::ResolveErrorKind::Msg(e) => Self::Message(e.to_string()),
            trust_dns_resolver::error::ResolveErrorKind::NoConnections => Self::NoConnections,
            trust_dns_resolver::error::ResolveErrorKind::NoRecordsFound { .. } => {
                Self::NoRecords {}
            }
            trust_dns_resolver::error::ResolveErrorKind::Io(io) => Self::IO(io.to_string()),
            trust_dns_resolver::error::ResolveErrorKind::Proto(proto) => {
                Self::Proto(proto.to_string())
            }
            trust_dns_resolver::error::ResolveErrorKind::Timeout => Self::TimedOut,
            // NOTE: non_exhaustive
            _ => Self::NotImplemented,
        }
    }
}
