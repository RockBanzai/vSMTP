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

use crate::smtp::{Sender, SenderHandler, UpgradeTls};
use crate::{Requirement, Tls};
use vsmtp_auth::TlsCertificate;
use vsmtp_common::delivery_attempt::{
    EitherEhloOrError, EitherGreetingsOrError, EitherRemoteServerOrError,
};
use vsmtp_common::{
    delivery_attempt::{
        DeliveryAttempt, RemoteInformation, RemoteMailExchange, RemoteServer, ShouldNotify,
    },
    extensions::Extension,
    response,
    stateful_ctx_received::MailFromProps,
    Recipient,
};
use vsmtp_protocol::{rustls, tokio_rustls, ClientName, Domain, Reader, Reply, Writer};

struct BasicSender {
    client_name: ClientName,
    message: Vec<u8>,
    mail_from: MailFromProps,
    rcpt_to: Vec<Recipient>,
    sni: rustls::ServerName,
    remote_output: RemoteInformation,
    tls: Tls,
    tls_connector: tokio_rustls::TlsConnector,
    should_notify: ShouldNotify,
}

#[async_trait::async_trait]
impl SenderHandler for BasicSender {
    type Result = DeliveryAttempt;

    async fn on_noop(&self, reply: Reply) -> Result<(), ()> {
        if reply.code().value() == 220 {
            Ok(())
        } else {
            Err(())
        }
    }

    async fn on_quit(&self, reply: Reply) -> Result<(), ()> {
        if reply.code().value() == 221 {
            Ok(())
        } else {
            Err(())
        }
    }

    fn has_just_connected(&self) -> bool {
        true
    }

    fn on_tls_upgrade_error(&mut self, error: std::io::Error) -> Self::Result {
        self.remote_output.save_tls_upgrade_error(error);
        self.take_result()
    }

    fn on_io_error(&mut self, error: vsmtp_protocol::Error) {
        self.remote_output.save_io_error(error);
    }

    async fn on_connect(&mut self) -> Result<(), ()> {
        Ok(())
    }

    async fn on_greetings(&mut self, reply: Reply) -> Result<(), ()> {
        if reply.code().value() == 220 {
            self.remote_output
                .save_greetings(EitherGreetingsOrError::Ok(reply));
            Ok(())
        } else {
            self.remote_output
                .save_greetings(EitherGreetingsOrError::Err((
                    "expect 220 on greetings".to_owned(),
                    reply,
                )));
            Err(())
        }
    }

    fn get_client_name(&self) -> ClientName {
        self.client_name.clone()
    }

    fn get_sni(&self) -> rustls::ServerName {
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

    fn get_tls_connector(&self) -> &tokio_rustls::TlsConnector {
        &self.tls_connector
    }

    async fn on_ehlo(&mut self, reply: Reply) -> Result<UpgradeTls, ()> {
        match response::Ehlo::try_from(reply.clone()) {
            Ok(response) => match (self.tls.starttls, response.contains(Extension::StartTls)) {
                (Requirement::Required, false) => {
                    self.remote_output.save_ehlo(EitherEhloOrError::Err((
                        "encrypted connection is required, but the server \
                                did not advertised STARTTLS extension"
                            .to_string(),
                        reply,
                    )));
                    Err(())
                }
                (Requirement::Required | Requirement::Optional, true) => {
                    self.remote_output
                        .save_ehlo(EitherEhloOrError::Ok(response));
                    Ok(UpgradeTls::Yes)
                }
                (Requirement::Optional, false) | (Requirement::Disabled, _) => {
                    self.remote_output
                        .save_ehlo(EitherEhloOrError::Ok(response));
                    Ok(UpgradeTls::No)
                }
            },
            Err(e) => {
                self.remote_output
                    .save_ehlo(EitherEhloOrError::Err((e.to_string(), reply)));
                Err(())
            }
        }
    }

    fn has_extension(&self, extension: Extension) -> bool {
        self.remote_output.has_extension(extension)
    }

    async fn on_mail_from(&mut self, reply: Reply) -> Result<(), ()> {
        self.remote_output.save_mail_from(reply);
        Ok(())
    }

    async fn on_rcpt_to(&mut self, _rcpt: &Recipient, reply: Reply) -> Result<(), ()> {
        self.remote_output.save_rcpt_to(reply);
        Ok(())
    }

    async fn on_data_start(&mut self, reply: Reply) -> Result<(), ()> {
        self.remote_output.save_data(reply);
        Ok(())
    }

    async fn on_data_end(&mut self, reply: Reply) -> Result<(), ()> {
        self.remote_output.save_data_end(reply);
        Ok(())
    }

    fn take_result(&mut self) -> DeliveryAttempt {
        DeliveryAttempt::new_remote(
            self.rcpt_to
                .iter()
                .map(|r| r.forward_path.clone())
                .collect(),
            self.remote_output.finalize(),
            self.should_notify,
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn send(
    ip_addr: std::net::SocketAddr,
    server_name: Domain,
    client_name: ClientName,
    from: MailFromProps,
    to: Vec<Recipient>,
    mx: Option<RemoteMailExchange>,
    message: &[u8],
    tls: Tls,
    extra_root_ca: Option<std::sync::Arc<TlsCertificate>>,
) -> DeliveryAttempt {
    let should_notify = ShouldNotify::Failure | ShouldNotify::Delay;

    let make_remote_information = |target| RemoteInformation::TcpConnection {
        mx,
        target,
        io: None,
    };

    let connect_timeout = std::time::Duration::from_secs(1);
    let socket = match tokio::time::timeout(
        connect_timeout,
        tokio::net::TcpStream::connect(ip_addr),
    )
    .await
    {
        Ok(socket) => match socket {
            Ok(socket) => socket,
            Err(error) => {
                return DeliveryAttempt::new_remote(
                    to.into_iter().map(|r| r.forward_path).collect(),
                    make_remote_information(EitherRemoteServerOrError::Err((
                        error.to_string(),
                        RemoteServer { ip_addr },
                    ))),
                    should_notify,
                );
            }
        },
        Err(elapsed) => {
            return DeliveryAttempt::new_remote(
                to.into_iter().map(|r| r.forward_path).collect(),
                make_remote_information(EitherRemoteServerOrError::Err((
                    format!("connection timeout reached: {elapsed}",),
                    RemoteServer { ip_addr },
                ))),
                should_notify,
            );
        }
    };

    // see https://man7.org/linux/man-pages/man2/getpeername.2.html
    let ip_addr = socket.peer_addr().expect("getpeername should never fail");
    let (read, write) = socket.into_split();

    let handler = BasicSender {
        client_name,
        sni: server_name.try_into().unwrap(),
        message: message.to_vec(),
        mail_from: from,
        rcpt_to: to.clone(),
        remote_output: make_remote_information(EitherRemoteServerOrError::Ok(RemoteServer {
            ip_addr,
        })),
        tls,
        should_notify,
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

    let Ok(pre_transaction) = sender.pre_transaction().await else {
        return sender.handler().take_result();
    };
    if matches!(pre_transaction, UpgradeTls::Yes) {
        match sender.upgrade_tls().await {
            Ok(mut secured_sender) => secured_sender.send().await,
            Err(info) => info,
        }
    } else {
        sender.send().await
    }
}
