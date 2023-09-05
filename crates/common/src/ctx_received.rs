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

use crate::faker::MailFaker;
use crate::{
    stateful_ctx_received::{CompleteProps, ConnectProps, HeloProps, MailFromProps, RcptToProps},
    DeserializeError, SerializeError,
};
use fake::Fake;
use vsmtp_mail_parser::Mail;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
#[serde(deny_unknown_fields)]
pub struct CtxReceived {
    pub connect: ConnectProps,
    pub helo: HeloProps,
    pub mail_from: MailFromProps,
    pub rcpt_to: RcptToProps,
    pub complete: CompleteProps,
    #[dummy(faker = "MailFaker")]
    #[serde(with = "crate::serde_helper::arc_rwlock")]
    pub mail: std::sync::Arc<std::sync::RwLock<Mail>>,
}

impl CtxReceived {
    pub fn to_json(&self) -> Result<Vec<u8>, DeserializeError> {
        match serde_json::to_vec(self) {
            Ok(this) => Ok(this),
            Err(err) => Err(DeserializeError::Error(err)),
        }
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, SerializeError> {
        match serde_json::from_slice(bytes) {
            Ok(this) => Ok(this),
            Err(err) => Err(SerializeError::Error(err)),
        }
    }

    #[must_use]
    pub fn fake() -> Self {
        fake::Faker.fake()
    }
}
