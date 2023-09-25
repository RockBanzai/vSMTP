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

use super::{Context, SenderHandler};
use crate::smtp::Sender;
use vsmtp_common::{
    delivery_attempt::{
        DeliveryAttempt, RemoteInformation, RemoteMailExchange, RemoteServer, ShouldNotify,
    },
    extensions::Extension,
    response,
    stateful_ctx_received::MailFromProps,
    transfer_error::Delivery,
    Recipient,
};
use vsmtp_protocol::{ClientName, Reader, Reply, Writer};

struct Handler {
    client_name: ClientName,
    message: Vec<u8>,
    mail_from: MailFromProps,
    rcpt_to: Vec<Recipient>,
    remote_output: RemoteInformation,
    should_notify: ShouldNotify,
}

#[async_trait::async_trait]
impl SenderHandler for Handler {
    fn has_just_connected(&self) -> bool {
        true
    }

    async fn on_connect(&mut self, _context: &mut Context) -> Result<(), Delivery> {
        Ok(())
    }

    async fn on_greetings(&mut self, reply: Reply) -> Result<(), Delivery> {
        if reply.code().value() != 220 {
            return Err(Delivery::ReplyParsing {
                with_source: Some("expect 220 on greetings".to_owned()),
            });
        }

        self.remote_output.save_greetings(reply);
        Ok(())
    }

    fn get_client_name(&self) -> ClientName {
        self.client_name.clone()
    }

    fn get_message(&self) -> Vec<u8> {
        self.message.clone()
    }

    fn get_mail_from(&self) -> MailFromProps {
        self.mail_from.clone()
    }

    fn get_rcpt_to(&self) -> Vec<Recipient> {
        self.rcpt_to.clone()
    }

    async fn on_ehlo(
        &mut self,
        response: response::Ehlo,
        _context: &mut Context,
    ) -> Result<(), Delivery> {
        self.remote_output.save_ehlo(response);
        Ok(())
    }

    fn has_extension(&self, extension: Extension) -> bool {
        self.remote_output.has_extension(extension)
    }

    async fn on_mail_from(&mut self, reply: Reply) -> Result<(), Delivery> {
        self.remote_output.save_mail_from(reply);
        Ok(())
    }

    async fn on_rcpt_to(&mut self, _rcpt: &Recipient, reply: Reply) -> Result<(), Delivery> {
        self.remote_output.save_rcpt_to(reply);
        Ok(())
    }

    async fn on_data_start(&mut self, reply: Reply) -> Result<(), Delivery> {
        self.remote_output.save_data(reply);
        Ok(())
    }

    async fn on_data_end(&mut self, reply: Reply) -> Result<(), Delivery> {
        self.remote_output.save_data_end(reply);
        Ok(())
    }

    fn get_result(&mut self) -> DeliveryAttempt {
        DeliveryAttempt::new_smtp(
            self.rcpt_to.clone(),
            self.remote_output.finalize(),
            self.should_notify.clone(),
        )
    }
}

pub async fn send(
    server: &str,
    port: u16,
    client_name: &str,
    from: MailFromProps,
    to: Vec<Recipient>,
    message: &[u8],
) -> DeliveryAttempt {
    let connect_timeout = std::time::Duration::from_secs(1);
    let socket = tokio::time::timeout(
        connect_timeout,
        tokio::net::TcpStream::connect((server, port)),
    )
    .await
    .unwrap()
    .unwrap();

    let remote_addr = socket.peer_addr().unwrap();

    let (read, write) = socket.into_split();

    let mut sender = Sender::new(
        Reader::new(Box::new(read), true),
        Writer::new(Box::new(write)),
        Handler {
            client_name: ClientName::Domain(client_name.parse().unwrap()),
            message: message.to_vec(),
            mail_from: from,
            rcpt_to: to,
            remote_output: RemoteInformation::IpLookup {
                mx: RemoteMailExchange {
                    mx: "example.com".parse().unwrap(),
                    mx_priority: 10,
                },
                target: RemoteServer {
                    ip_addr: remote_addr,
                },
            },
            should_notify: ShouldNotify {
                // false only if the DSN has been transferred to the next hop
                on_success: false,
                on_failure: true,
                on_delay: true,
                on_expanded: false,
                on_relayed: false,
            },
        },
    );

    let mut context = Context;

    if let Err(_e) = sender.pre_transaction(&mut context).await {
        sender.handler().get_result()
    } else {
        sender.send().await
    }
}
