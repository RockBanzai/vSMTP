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

use super::{handler::UpgradeTls, Sender, SenderHandler};
use crate::{Requirement, Tls};
use vsmtp_auth::TlsCertificate;
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
use vsmtp_protocol::{rustls, tokio_rustls, ClientName, Domain, Reader, Reply, Writer};

struct Handler {
    client_name: ClientName,
    message: Vec<u8>,
    mail_from: MailFromProps,
    rcpt_to: Vec<Recipient>,
    sni: vsmtp_protocol::rustls::ServerName,
    remote_output: RemoteInformation,
    tls: Tls,
    tls_connector: vsmtp_protocol::tokio_rustls::TlsConnector,
    should_notify: ShouldNotify,
}

#[async_trait::async_trait]
impl SenderHandler for Handler {
    fn has_just_connected(&self) -> bool {
        true
    }

    async fn on_connect(&mut self) -> Result<(), Delivery> {
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

    fn get_sni(&self) -> vsmtp_protocol::rustls::ServerName {
        self.sni.clone()
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

    fn get_tls_connector(&self) -> &vsmtp_protocol::tokio_rustls::TlsConnector {
        &self.tls_connector
    }

    async fn on_ehlo(&mut self, response: response::Ehlo) -> Result<UpgradeTls, Delivery> {
        self.remote_output.save_ehlo(response);

        let has_starttls = self.remote_output.has_extension(Extension::StartTls);

        match self.tls.starttls {
            Requirement::Required if !has_starttls => Err(Delivery::StartTls),
            Requirement::Optional if !has_starttls => Ok(UpgradeTls::No),
            Requirement::Required | Requirement::Optional => Ok(UpgradeTls::Yes),
            Requirement::Disabled => Ok(UpgradeTls::No),
        }
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

#[allow(clippy::too_many_arguments)]
pub async fn send(
    server: &str,
    domain: Domain,
    port: u16,
    client_name: &str,
    from: MailFromProps,
    to: Vec<Recipient>,
    message: &[u8],
    tls: &Tls,
    extra_root_ca: Option<std::sync::Arc<TlsCertificate>>,
) -> DeliveryAttempt {
    let should_notify = ShouldNotify {
        // false only if the DSN has been transferred to the next hop
        on_success: false,
        on_failure: true,
        on_delay: true,
        on_expanded: false,
        on_relayed: false,
    };
    let connect_timeout = std::time::Duration::from_secs(1);
    let socket = match tokio::time::timeout(
        connect_timeout,
        tokio::net::TcpStream::connect((server, port)),
    )
    .await
    {
        Ok(socket) => match socket {
            Ok(socket) => socket,
            Err(error) => {
                return DeliveryAttempt::new_smtp(
                    to,
                    RemoteInformation::ConnectError {
                        error: error.to_string(),
                    },
                    should_notify,
                )
            }
        },
        Err(_) => {
            return DeliveryAttempt::new_smtp(
                to,
                RemoteInformation::ConnectError {
                    error: "connection timeout reached".to_string(),
                },
                should_notify,
            );
        }
    };

    let remote_addr = socket.peer_addr().unwrap();
    let (read, write) = socket.into_split();
    let handler = Handler {
        client_name: ClientName::Domain(client_name.parse().unwrap()),
        sni: rustls::ServerName::try_from(domain.to_string().as_str())
            .expect("valid domain from the trust-dns crate"),
        message: message.to_vec(),
        mail_from: from,
        rcpt_to: to.clone(),
        remote_output: RemoteInformation::IpLookup {
            mx: RemoteMailExchange {
                mx: "example.com".parse().unwrap(),
                mx_priority: 10,
            },
            target: RemoteServer {
                ip_addr: remote_addr,
            },
        },
        tls: tls.clone(),
        should_notify: should_notify.clone(),
        // TODO: should be cached.
        tls_connector: {
            let mut root_store = rustls::RootCertStore::empty();

            root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
                rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            }));

            if let Some(extra_root_ca) = extra_root_ca {
                for i in extra_root_ca.certs() {
                    root_store.add(i).unwrap();
                }
            }

            // NOTE: We could let the user customize the tls parameters here.
            let config = rustls::ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_store)
                .with_no_client_auth();

            tokio_rustls::TlsConnector::from(std::sync::Arc::new(config))
        },
    };

    let mut sender = Sender::new(
        Reader::new(Box::new(read), true),
        Writer::new(Box::new(write)),
        handler,
    );

    if matches!(sender.pre_transaction().await.unwrap(), UpgradeTls::Yes) {
        let mut sender = sender.upgrade_tls().await.unwrap();
        sender.send().await
    } else {
        sender.send().await
    }
}
