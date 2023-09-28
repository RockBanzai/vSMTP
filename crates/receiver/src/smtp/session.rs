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

use super::{
    config::{Esmtp, SMTPReceiverConfig},
    rules::{stages::ReceiverStage, status::ReceiverStatus},
};
use futures_util::stream::TryStreamExt;
use vsmtp_common::{
    api::{write_to_quarantine, write_to_working},
    ctx_received::CtxReceived,
    delivery_route::DeliveryRoute,
    extensions::Extension,
    stateful_ctx_received::{ConnectProps, SaslAuthProps, StatefulCtxReceived},
    Mailbox, Recipient,
};
use vsmtp_mail_parser::ParserError;
use vsmtp_protocol::{
    rsasl, rustls, AcceptArgs, AuthArgs, AuthError, ClientName, ConnectionKind, Domain, EhloArgs,
    Error, HeloArgs, MailFromArgs, ParseArgsError, RcptToArgs, ReceiverContext, Reply, Stage,
};
use vsmtp_rule_engine::{RuleEngine, RuleEngineConfig};

pub struct Handler {
    rule_engine: std::sync::Arc<RuleEngine<StatefulCtxReceived, ReceiverStatus, ReceiverStage>>,
    going_to_quarantine: Option<String>,
    channel: lapin::Channel,
    // receiver config
    config: std::sync::Arc<SMTPReceiverConfig>,
    rustls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
}

fn default_deny() -> Reply {
    "554 permanent problems with the remote server\r\n"
        .parse::<Reply>()
        .unwrap()
}

fn default_accept() -> Reply {
    "250 Ok\r\n".parse::<Reply>().unwrap()
}

fn convert_error(e: Error) -> ParserError {
    if e.get_ref().is_some() {
        match e.into_inner().unwrap().downcast::<std::io::Error>() {
            Ok(io) => ParserError::Io(*io),
            Err(otherwise) => match otherwise.downcast::<ParseArgsError>().map(|i| *i) {
                Ok(ParseArgsError::BufferTooLong { expected, got }) => {
                    ParserError::BufferTooLong { expected, got }
                }
                Ok(otherwise) => ParserError::InvalidMail(otherwise.to_string()),
                Err(otherwise) => ParserError::InvalidMail(otherwise.to_string()),
            },
        }
    } else {
        ParserError::InvalidMail(e.to_string())
    }
}

impl Handler {
    pub fn on_accept(
        AcceptArgs {
            client_addr,
            server_addr,
            timestamp,
            uuid,
            kind,
            ..
        }: AcceptArgs,
        rule_engine_config: std::sync::Arc<
            RuleEngineConfig<StatefulCtxReceived, ReceiverStatus, ReceiverStage>,
        >,
        channel: lapin::Channel,
        config: std::sync::Arc<SMTPReceiverConfig>,
        rustls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    ) -> (Self, ReceiverContext, Option<Reply>) {
        let mut ctx = ReceiverContext::default();

        let server_name = hostname::get()
            .unwrap()
            .to_string_lossy()
            .parse::<Domain>()
            .unwrap();

        let rule_engine = RuleEngine::from_config_with_state(
            rule_engine_config,
            StatefulCtxReceived::new(ConnectProps {
                client_addr,
                server_addr,
                server_name: server_name.clone(),
                connect_timestamp: timestamp,
                connect_uuid: uuid,
                sasl: None,
                iprev: None,
                tls: None,
            }),
        );

        let default = || {
            format!("220 {server_name} Service ready\r\n")
                .parse::<Reply>()
                .unwrap()
        };

        let status = rule_engine.run(&ReceiverStage::Connect);

        let config_clone = config.clone();
        let rustls_config_clone = rustls_config.clone();
        let make = |going_to_quarantine| Self {
            rule_engine: rule_engine.into(),
            going_to_quarantine,
            channel,
            config: config_clone,
            rustls_config: rustls_config_clone,
        };

        // NOTE: The rule engine result is ignored in this case ...
        if kind == ConnectionKind::Tunneled {
            match (rustls_config, config.tls.as_ref()) {
                (Some(rustls_config), Some(tls_config)) => {
                    ctx.upgrade_tls(rustls_config, tls_config.handshake_timeout);
                }
                // Tunneled connection without TLS config is not allowed.
                _ => ctx.deny(),
            };

            return (make(None), ctx, None);
        }

        // NOTE: do we want to allow the user to override the reply on accept?
        match status {
            ReceiverStatus::Next => (make(None), ctx, Some(default())),
            ReceiverStatus::Accept(reply) => {
                (make(None), ctx, Some(reply.unwrap_or_else(default_accept)))
            }
            ReceiverStatus::Deny(reply) => {
                ctx.deny();
                (make(None), ctx, Some(reply.unwrap_or_else(default_deny)))
            }
            ReceiverStatus::Quarantine(name, reply) => {
                (make(Some(name)), ctx, Some(reply.unwrap_or_else(default)))
            }
        }
    }
}

struct RsaslSessionCallback {
    rule_engine: std::sync::Arc<RuleEngine<StatefulCtxReceived, ReceiverStatus, ReceiverStage>>,
}

pub struct SaslValidation;

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error(
        "the rules at stage '{}' returned non 'accept' status",
        ReceiverStage::Authenticate
    )]
    NonAcceptCode,
}

impl rsasl::validate::Validation for SaslValidation {
    type Value = ();
}

impl rsasl::callback::SessionCallback for RsaslSessionCallback {
    fn callback(
        &self,
        _session_data: &rsasl::callback::SessionData,
        _context: &rsasl::callback::Context<'_>,
        _request: &mut rsasl::callback::Request<'_>,
    ) -> Result<(), rsasl::prelude::SessionError> {
        Ok(())
    }

    fn validate(
        &self,
        session_data: &rsasl::callback::SessionData,
        context: &rsasl::callback::Context<'_>,
        validate: &mut rsasl::validate::Validate<'_>,
    ) -> Result<(), rsasl::validate::ValidationError> {
        let credentials = vsmtp_protocol::auth::Credentials::try_from((session_data, context))
            .map_err(|e| match e {
                vsmtp_protocol::auth::Error::MissingField => {
                    rsasl::validate::ValidationError::MissingRequiredProperty
                }
                otherwise => rsasl::validate::ValidationError::Boxed(Box::new(otherwise)),
            })?;

        validate.with::<SaslValidation, _>(|| {
            self.rule_engine.write_state(|i| {
                i.mut_connect().sasl = Some(SaslAuthProps {
                    mechanism: session_data.mechanism().to_string().parse().unwrap(),
                    cancel_count: 0,
                    is_authenticated: false,
                    credentials,
                });
            });
            let result = self.rule_engine.run(&ReceiverStage::Authenticate);

            if matches!(result, ReceiverStatus::Accept(_)) {
                Ok(())
            } else {
                Err(rsasl::validate::ValidationError::Boxed(Box::new(
                    ValidationError::NonAcceptCode,
                )))
            }
        })?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl vsmtp_protocol::ReceiverHandler for Handler {
    type Item = (CtxReceived, Option<String>);

    fn get_stage(&self) -> Stage {
        self.rule_engine.read_state(StatefulCtxReceived::get_stage)
    }

    fn generate_sasl_callback(&self) -> vsmtp_protocol::CallbackWrap {
        vsmtp_protocol::CallbackWrap(Box::new(RsaslSessionCallback {
            rule_engine: self.rule_engine.clone(),
        }))
    }

    async fn on_post_tls_handshake(
        &mut self,
        sni: Option<String>,
        protocol_version: rustls::ProtocolVersion,
        cipher_suite: rustls::CipherSuite,
        peer_certificates: Option<Vec<rustls::Certificate>>,
        alpn_protocol: Option<Vec<u8>>,
    ) -> Reply {
        match self.rule_engine.write_state(|state| {
            // FIXME: should return an error instead of ignoring the SNI if could not been parsed ?
            let sni = sni
                .clone()
                .and_then(|sni| <Domain as std::str::FromStr>::from_str(&sni).ok());

            state.set_secured(
                sni,
                protocol_version,
                cipher_suite,
                peer_certificates,
                alpn_protocol,
            )
        }) {
            Ok(()) => format!(
                "220 {} Service ready\r\n",
                sni.unwrap_or_else(|| self.config.name.clone())
            )
            .parse::<Reply>()
            .unwrap(),
            Err(error) => {
                tracing::warn!(%error, "Post TLS handshake called during a wrong stage");
                "451 Requested action aborted: error in processing.\r\n"
                    .parse::<Reply>()
                    .unwrap()
            }
        }
    }

    async fn on_starttls(&mut self, ctx: &mut ReceiverContext) -> Reply {
        if !self.config.esmtp.starttls {
            // https://www.ietf.org/rfc/rfc5321.txt#4.2.4
            "502 Command not implemented\r\n".parse::<Reply>().unwrap()
        } else if self.rule_engine.read_state(StatefulCtxReceived::is_secured) {
            "554 5.5.1 Error: TLS already active\r\n"
                .parse::<Reply>()
                .unwrap()
        } else {
            match (self.rustls_config.as_ref(), self.config.tls.as_ref()) {
                (Some(rustls_config), Some(tls_config)) => {
                    ctx.upgrade_tls(rustls_config.clone(), tls_config.handshake_timeout);
                    "220 Ready to start TLS\r\n".parse::<Reply>().unwrap()
                }
                _ => "454 TLS not available due to temporary reason\r\n"
                    .parse::<Reply>()
                    .unwrap(),
            }
        }
    }

    // TODO: handle "538 5.7.11 Encryption required for requested authentication mechanism\r\n"
    async fn on_auth(
        &mut self,
        ctx: &mut ReceiverContext,
        AuthArgs {
            mechanism,
            initial_response,
            ..
        }: AuthArgs,
    ) -> Option<Reply> {
        ctx.authenticate(mechanism, initial_response);
        None
    }

    async fn on_post_auth(
        &mut self,
        ctx: &mut ReceiverContext,
        result: Result<(), AuthError>,
    ) -> Reply {
        match result {
            Ok(()) => {
                self.rule_engine.write_state(|i| {
                    i.mut_connect()
                        .sasl
                        .as_mut()
                        .expect("auth props has been set before/during the SASL handshake")
                        .is_authenticated = true;
                });

                "235 2.7.0 Authentication succeeded\r\n"
                    .parse::<Reply>()
                    .unwrap()
            }
            Err(AuthError::ClientMustNotStart) => {
                "501 5.7.0 Client must not start with this mechanism\r\n"
                    .parse::<Reply>()
                    .unwrap()
            }
            Err(AuthError::ValidationError(..)) => {
                ctx.deny();
                "535 5.7.8 Authentication credentials invalid\r\n"
                    .parse::<Reply>()
                    .unwrap()
            }
            Err(AuthError::Canceled) => self.rule_engine.write_state(|i| {
                let auth_props = i
                    .mut_connect()
                    .sasl
                    .as_mut()
                    .expect("auth props has been set before/during the SASL handshake");

                auth_props.cancel_count += 1;
                // TODO: put the SASL handshake cancel count into the config
                let attempt_count_max = -1;
                /*
                let attempt_count_max = self
                    .config
                    .server
                    .esmtp
                    .auth
                    .as_ref()
                    .map_or(-1, |auth| auth.attempt_count_max);
                */

                if attempt_count_max != -1
                    && auth_props.cancel_count >= attempt_count_max.try_into().unwrap()
                {
                    ctx.deny();
                }

                "501 Authentication canceled by client\r\n"
                    .parse::<Reply>()
                    .unwrap()
            }),
            Err(AuthError::Base64 { .. }) => "501 5.5.2 Invalid, not base64\r\n"
                .parse::<Reply>()
                .unwrap(),
            Err(AuthError::SessionError(e)) => {
                tracing::warn!(%e, "auth error");
                ctx.deny();
                "454 4.7.0 Temporary authentication failure\r\n"
                    .parse::<Reply>()
                    .unwrap()
            }
            Err(AuthError::IO(e)) => todo!("io auth error {e}"),
            Err(AuthError::ConfigError(rsasl::prelude::SASLError::NoSharedMechanism)) => {
                ctx.deny();
                "504 5.5.4 Mechanism is not supported\r\n"
                    .parse::<Reply>()
                    .unwrap()
            }
            Err(AuthError::ConfigError(e)) => todo!("handle non_exhaustive pattern: {e}"),
        }
    }

    async fn on_helo(
        &mut self,
        ctx: &mut ReceiverContext,
        HeloArgs { client_name, .. }: HeloArgs,
    ) -> Reply {
        let default = {
            let client_name = client_name.clone();
            let server_name = self.rule_engine.read_state(|i| i.server_name().clone());

            move || {
                format!("250 {server_name} Greetings {client_name}\r\n",)
                    .parse()
                    .unwrap()
            }
        };

        if let Err(error) = self.rule_engine.write_state(|i| {
            i.set_helo(ClientName::Domain(client_name), true)
                .map(|_| ())
        }) {
            tracing::debug!(?error, "Client sent bad HELO/EHLO command");
            return "503 Bad sequence of commands\r\n".parse().unwrap();
        }

        // NOTE: do we want to allow the user to override the reply on helo?
        match self.rule_engine.run(&ReceiverStage::Helo) {
            ReceiverStatus::Next => default(),
            ReceiverStatus::Accept(reply) => reply.unwrap_or_else(default_accept),
            ReceiverStatus::Deny(reply) => {
                ctx.deny();
                reply.unwrap_or_else(default_deny)
            }
            ReceiverStatus::Quarantine(name, reply) => {
                self.going_to_quarantine = Some(name);
                reply.unwrap_or_else(default)
            }
        }
    }

    async fn on_ehlo(
        &mut self,
        ctx: &mut ReceiverContext,
        EhloArgs { client_name, .. }: EhloArgs,
    ) -> Reply {
        let default = self.build_ehlo_reply(&client_name);
        // NOTE: do we want to allow the user to override the reply on ehlo?
        match self.rule_engine.run(&ReceiverStage::Helo) {
            ReceiverStatus::Next => default,
            ReceiverStatus::Accept(reply) => reply.unwrap_or_else(default_accept),
            ReceiverStatus::Deny(reply) => {
                ctx.deny();
                reply.unwrap_or_else(default_deny)
            }
            ReceiverStatus::Quarantine(name, reply) => {
                self.going_to_quarantine = Some(name);
                reply.unwrap_or(default)
            }
        }
    }

    async fn on_mail_from(
        &mut self,
        ctx: &mut ReceiverContext,
        MailFromArgs {
            reverse_path,
            envelop_id,
            ret,
            ..
        }: MailFromArgs,
    ) -> Reply {
        let reverse_path = reverse_path.map(Mailbox);
        let default: Reply = reverse_path
            .as_ref()
            .map_or_else(
                || "250 sender <> Ok".to_string(),
                |reverse_path| format!("250 sender <{reverse_path}> Ok"),
            )
            .parse()
            .unwrap();

        self.rule_engine.write_state(|i| {
            i.set_mail_from(reverse_path, envelop_id, ret).unwrap();
        });

        match self.rule_engine.run(&ReceiverStage::MailFrom) {
            ReceiverStatus::Next => default,
            ReceiverStatus::Accept(reply) => reply.unwrap_or_else(default_accept),
            ReceiverStatus::Deny(reply) => {
                ctx.deny();
                reply.unwrap_or_else(default_deny)
            }
            ReceiverStatus::Quarantine(name, reply) => {
                self.going_to_quarantine = Some(name);
                reply.unwrap_or(default)
            }
        }
    }

    async fn on_rcpt_to(
        &mut self,
        ctx: &mut ReceiverContext,
        RcptToArgs {
            forward_path,
            original_forward_path,
            notify_on,
            ..
        }: RcptToArgs,
    ) -> Reply {
        // TODO: add too much rcpt

        let default = format!("250 recipient <{forward_path}> Ok")
            .parse::<Reply>()
            .unwrap();

        let route = DeliveryRoute::Basic;
        self.rule_engine.write_state(|i| {
            i.set_rcpt_to(
                route,
                Recipient {
                    forward_path: Mailbox(forward_path),
                    original_forward_path,
                    notify_on,
                },
            )
            .unwrap();
        });

        match self.rule_engine.run(&ReceiverStage::RcptTo) {
            ReceiverStatus::Next => default,
            ReceiverStatus::Accept(reply) => reply.unwrap_or_else(default_accept),
            ReceiverStatus::Deny(reply) => {
                ctx.deny();
                reply.unwrap_or_else(default_deny)
            }
            ReceiverStatus::Quarantine(name, reply) => {
                self.going_to_quarantine = Some(name);
                reply.unwrap_or(default)
            }
        }
    }

    async fn on_rset(&mut self) -> Reply {
        self.rule_engine.write_state(StatefulCtxReceived::reset);
        self.going_to_quarantine = None;

        "250 Ok\r\n".parse::<Reply>().unwrap()
    }

    async fn on_message(
        &mut self,
        ctx: &mut ReceiverContext,
        stream: impl tokio_stream::Stream<Item = Result<Vec<u8>, vsmtp_protocol::Error>> + Send + Unpin,
        // FIXME: output should be just one Self::Item and not a vec
    ) -> (Reply, Option<Vec<Self::Item>>) {
        let mail = {
            tracing::debug!("SMTP handshake completed");
            let stream = stream.map_err(convert_error);

            // FIXME: the message_size max is already defined when instantiating the `proto::Receiver`
            let mail = match vsmtp_mail_parser::Mail::parse_stream(stream).await {
                Ok(mail) => mail,
                Err(ParserError::BufferTooLong { .. }) => {
                    return (
                        "552 4.3.1 Message size exceeds fixed maximum message size\r\n"
                            .parse::<Reply>()
                            .unwrap(),
                        None,
                    );
                }
                Err(otherwise) => todo!("handle error cleanly {:?}", otherwise),
            };
            tracing::debug!("Message body fully received");
            mail
        };

        // TODO: add headers from preq rules

        let message_size = mail.to_string().len();
        self.rule_engine.write_state(|i| {
            i.set_complete(mail).unwrap();
        });

        let default = || {
            format!("250 message of {message_size} bytes Ok")
                .parse()
                .unwrap()
        };

        let (reply, should_return) = match self.rule_engine.run(&ReceiverStage::PreQueue) {
            ReceiverStatus::Next => (default(), true),
            ReceiverStatus::Accept(reply) => (reply.unwrap_or_else(default_accept), true),
            ReceiverStatus::Deny(reply) => {
                ctx.deny();
                (reply.unwrap_or_else(default_deny), false)
            }
            ReceiverStatus::Quarantine(name, reply) => {
                self.going_to_quarantine = Some(name);
                (reply.unwrap_or_else(default), true)
            }
        };

        let Self {
            rule_engine,
            going_to_quarantine,
            channel: _,
            config: _,
            rustls_config: _,
        } = self;

        let ctx: CtxReceived =
            rule_engine.write_state(|i| std::mem::replace(i, i.produce_new()).try_into().unwrap());
        let going_to_quarantine = std::mem::take(going_to_quarantine);

        (
            reply,
            should_return.then(|| vec![(ctx, going_to_quarantine)]),
        )
    }

    async fn on_message_completed(&mut self, item: Self::Item) -> Option<Reply> {
        // TODO: handle timeout and all the amqp errors
        // let timeout_duration = std::time::Duration::from_secs(5);

        let (ctx, going_to_quarantine) = item;
        let payload = ctx.to_json().unwrap();

        if let Some(quarantine) = going_to_quarantine {
            tracing::debug!("Sending to quarantine('{}')...", quarantine);
            write_to_quarantine(&self.channel, &quarantine, payload).await;
        } else {
            tracing::debug!("Sending to working...");
            write_to_working(&self.channel, payload).await;
        }

        None
    }

    async fn on_hard_error(&mut self, ctx: &mut ReceiverContext, reply: Reply) -> Reply {
        ctx.deny();
        reply.extended(
            &"451 Too many errors from the client\r\n"
                .parse::<Reply>()
                .unwrap(),
        )
    }

    async fn on_soft_error(&mut self, _: &mut ReceiverContext, reply: Reply) -> Reply {
        // TODO: configurable
        // self.config.server.smtp.error.delay
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        reply
    }
}

impl Handler {
    fn build_ehlo_reply(&mut self, client_name: &ClientName) -> Reply {
        self.rule_engine.write_state(|i| {
            if let Err(error) = i.set_helo(client_name.clone(), false) {
                tracing::debug!(?error, "Client sent bad HELO/EHLO command");
                return "503 Bad sequence of commands\r\n".parse().unwrap();
            }

            let Esmtp {
                auth,
                starttls,
                pipelining,
                size: _,
                dsn,
            } = &self.config.esmtp;

            [
                Some(format!(
                    "250-{} Greetings {}\r\n",
                    i.server_name(),
                    client_name
                )),
                Some(format!("250-{}\r\n", Extension::EnhancedStatusCodes)),
                pipelining.then(|| format!("250-{}\r\n", Extension::Pipelining)),
                dsn.then(|| format!("250-{}\r\n", Extension::DeliveryStatusNotification)),
                if *starttls {
                    if self.config.tls.is_some() {
                        Some(format!("250-{}\r\n", Extension::StartTls))
                    } else {
                        tracing::warn!("STARTTLS is enabled but TLS is not configured");
                        None
                    }
                } else {
                    None
                },
                auth.as_ref().map(|auth| {
                    format!(
                        "250-{} {}\r\n",
                        Extension::Auth,
                        auth.mechanisms
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                }),
                Some("250 \r\n".to_string()),
            ]
            .into_iter()
            .flatten()
            .collect::<String>()
            .parse()
            .expect("EHLO must be valid")
        })
    }
}
