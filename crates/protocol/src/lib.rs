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

extern crate alloc;

mod command;
mod connection_kind;
mod error;
mod reader;
mod receiver;
mod receiver_handler;

mod smtp_sasl;
mod writer;

mod stage;
pub use stage::Stage;

pub mod auth {
    mod credentials;
    mod mechanism;

    pub use credentials::{Credentials, Error};
    pub use mechanism::Mechanism;
}

mod types {
    pub mod address;
    pub mod client_name;
    pub mod domain;
    pub mod reply;
    pub mod reply_code;
}

pub use command::{
    AcceptArgs, AuthArgs, DsnReturn, EhloArgs, HeloArgs, MailFromArgs, NotifyOn, OriginalRecipient,
    RcptToArgs, UnparsedArgs, Verb,
};
pub use connection_kind::ConnectionKind;
pub use error::{Error, ErrorKind, ParseArgsError};
pub use reader::Reader;
pub use receiver::{Receiver, ReceiverContext};
pub use receiver_handler::ReceiverHandler;
pub use rsasl;
pub use smtp_sasl::{AuthError, CallbackWrap};
pub use tokio_rustls;
pub use tokio_rustls::rustls;
pub use types::{
    address::Address, client_name::ClientName, domain::Domain, reply::Reply, reply_code::ReplyCode,
};
pub use writer::Writer;
