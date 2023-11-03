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

use vsmtp_common::{extensions::Extension, stateful_ctx_received::MailFromProps, Recipient};
use vsmtp_protocol::{rustls, tokio_rustls, ClientName, Reply};

pub enum UpgradeTls {
    Yes,
    No,
}

#[async_trait::async_trait]
pub trait SenderHandler {
    type Result: std::fmt::Debug;

    fn has_just_connected(&self) -> bool;

    fn get_client_name(&self) -> ClientName;
    fn get_sni(&self) -> rustls::ServerName;
    fn get_message(&self) -> Vec<u8>;
    fn get_mail_from(&self) -> MailFromProps;
    fn get_rcpt_to(&self) -> Vec<Recipient>;
    fn get_tls_connector(&self) -> &tokio_rustls::TlsConnector;

    fn has_extension(&self, extension: Extension) -> bool;

    fn has_pipelining(&self) -> bool {
        self.has_extension(Extension::Pipelining)
    }

    fn has_dsn(&self) -> bool {
        self.has_extension(Extension::DeliveryStatusNotification)
    }

    async fn on_noop(&self, reply: Reply) -> Result<(), ()>;
    async fn on_quit(&self, reply: Reply) -> Result<(), ()>;
    async fn on_connect(&mut self) -> Result<(), ()>;
    async fn on_greetings(&mut self, reply: Reply) -> Result<(), ()>;
    async fn on_ehlo(&mut self, reply: Reply) -> Result<UpgradeTls, ()>;
    async fn on_mail_from(&mut self, reply: Reply) -> Result<(), ()>;
    async fn on_rcpt_to(&mut self, rcpt: &Recipient, reply: Reply) -> Result<(), ()>;
    async fn on_data_start(&mut self, reply: Reply) -> Result<(), ()>;
    async fn on_data_end(&mut self, reply: Reply) -> Result<(), ()>;

    fn on_tls_upgrade_error(&mut self, error: std::io::Error) -> Self::Result;
    fn on_io_error(&mut self, error: vsmtp_protocol::Error);

    fn take_result(&mut self) -> Self::Result;
}
