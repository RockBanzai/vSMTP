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
    delivery_attempt::DeliveryAttempt, delivery_route::DeliveryRoute, faker::DeliveryRouteFaker,
    stateful_ctx_received::MailFromProps, DeserializeError, Recipient, SerializeError,
};
use fake::Fake;
use vsmtp_mail_parser::Mail;

#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct CtxDelivery {
    pub uuid: uuid::Uuid,
    #[dummy(faker = "DeliveryRouteFaker { r#type: None }")]
    pub routing_key: DeliveryRoute,
    pub mail_from: MailFromProps,
    pub rcpt_to: Vec<Recipient>,
    #[dummy(faker = "MailFaker")]
    pub mail: std::sync::Arc<std::sync::RwLock<Mail>>,
    pub last_deliveries: Vec<DeliveryAttempt>,
    pub attempt: Vec<DeliveryAttempt>,
}

impl CtxDelivery {
    pub fn new(
        route: DeliveryRoute,
        mail_from: MailFromProps,
        rcpt_to: Vec<Recipient>,
        mail: std::sync::Arc<std::sync::RwLock<Mail>>,
    ) -> Self {
        Self {
            uuid: uuid::Uuid::new_v4(),
            routing_key: route,
            mail_from,
            rcpt_to,
            mail,
            last_deliveries: vec![],
            attempt: vec![],
        }
    }

    #[must_use]
    pub fn get_delayed_duration(&self) -> std::time::Duration {
        // should be exp or something
        std::time::Duration::from_secs((self.attempt.len() * 10).try_into().unwrap())
    }

    pub fn get_undelivered_rcpt(&self) -> impl Iterator<Item = &Recipient> {
        fn recipient_attempt_is_successful(attempt: &DeliveryAttempt, rcpt: &Recipient) -> bool {
            attempt
                .get_rcpt_index(rcpt)
                .is_some_and(|rcpt_idx| attempt.get_action(rcpt_idx).is_successful())
        }

        self.rcpt_to.iter().filter(|rcpt| {
            !self
                .attempt
                .iter()
                .rev()
                .any(|attempt| recipient_attempt_is_successful(attempt, rcpt))
        })
    }

    #[must_use]
    pub fn get_last_delivery_attempt_of_rcpt(
        &self,
        recipient: &Recipient,
    ) -> Option<(&DeliveryAttempt, usize)> {
        self.last_deliveries
            .iter()
            .find_map(|attempt| attempt.get_rcpt_index(recipient).map(|idx| (attempt, idx)))
    }

    /// A message is fully delivered if all recipients have been delivered successfully
    #[must_use]
    pub fn is_fully_delivered(&self) -> bool {
        self.get_undelivered_rcpt().count() == 0
    }

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
