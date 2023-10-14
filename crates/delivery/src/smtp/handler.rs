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

use vsmtp_common::{
    delivery_attempt::DeliveryAttempt, extensions::Extension, response,
    stateful_ctx_received::MailFromProps, transfer_error::Delivery, Recipient,
};
use vsmtp_protocol::{ClientName, Reply};

pub enum UpgradeTls {
    Yes,
    No,
}

#[async_trait::async_trait]
pub trait SenderHandler {
    // NOTE: noop response could be parsed by the sender, so ill-formed response are not a problem
    async fn on_noop(&self, reply: Reply) -> Result<(), Delivery> {
        if reply.code().value() != 220 {
            return Err(Delivery::ReplyParsing {
                with_source: Some("expect 220 on noop".to_owned()),
            });
        }

        Ok(())
    }

    async fn on_quit(&self, reply: Reply) -> Result<(), Delivery> {
        if reply.code().value() != 221 {
            return Err(Delivery::ReplyParsing {
                with_source: Some("expect 221 on quit".to_owned()),
            });
        }

        Ok(())
    }

    fn has_just_connected(&self) -> bool;

    async fn on_connect(&mut self) -> Result<(), Delivery>;
    async fn on_greetings(&mut self, reply: Reply) -> Result<(), Delivery>;

    fn get_client_name(&self) -> ClientName;
    fn get_sni(&self) -> vsmtp_protocol::rustls::ServerName;
    fn get_message(&self) -> Vec<u8>;
    fn get_mail_from(&self) -> MailFromProps;
    fn get_rcpt_to(&self) -> Vec<Recipient>;
    fn get_tls_connector(&self) -> &vsmtp_protocol::tokio_rustls::TlsConnector;

    async fn on_ehlo(&mut self, response: response::Ehlo) -> Result<UpgradeTls, Delivery>;

    fn has_extension(&self, extension: Extension) -> bool;

    fn has_pipelining(&self) -> bool {
        self.has_extension(Extension::Pipelining)
    }

    fn has_dsn(&self) -> bool {
        self.has_extension(Extension::DeliveryStatusNotification)
    }

    async fn on_mail_from(&mut self, reply: Reply) -> Result<(), Delivery>;
    async fn on_rcpt_to(&mut self, rcpt: &Recipient, reply: Reply) -> Result<(), Delivery>;
    async fn on_data_start(&mut self, reply: Reply) -> Result<(), Delivery>;
    async fn on_data_end(&mut self, reply: Reply) -> Result<(), Delivery>;

    fn get_result(&mut self) -> DeliveryAttempt;
}
