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

use super::SenderHandler;
use crate::smtp::handler::UpgradeTls;
use vsmtp_common::{stateful_ctx_received::MailFromProps, Recipient};
use vsmtp_protocol::{DsnReturn, NotifyOn, Reader, Reply, Verb, Writer};

pub struct Sender<H: SenderHandler> {
    reader: Reader<Box<dyn tokio::io::AsyncRead + Unpin + Send + Sync>>,
    writer: Writer<Box<dyn tokio::io::AsyncWrite + Unpin + Send + Sync>>,
    handler: H,
}
impl<H: SenderHandler + Sync + Send> Sender<H> {
    pub fn new(
        reader: Reader<Box<dyn tokio::io::AsyncRead + Unpin + Send + Sync>>,
        writer: Writer<Box<dyn tokio::io::AsyncWrite + Unpin + Send + Sync>>,
        handler: H,
    ) -> Self {
        Self {
            reader,
            writer,
            handler,
        }
    }

    pub fn handler(&mut self) -> &mut H {
        &mut self.handler
    }

    pub async fn quit(&mut self) -> Result<(), ()> {
        if let Err(e) = self.writer.write_all(Verb::Quit.as_ref()).await {
            self.handler.on_io_error(e.into());
            return Err(());
        }

        let replies = self.reader.as_reply_stream();
        tokio::pin!(replies);

        match Self::next_reply(&mut replies).await {
            Ok(reply) => self.handler.on_quit(reply).await,
            Err(e) => {
                self.handler.on_io_error(e);
                Err(())
            }
        }
    }

    pub async fn noop(&mut self) -> Result<(), ()> {
        if let Err(e) = self.writer.write_all(Verb::Noop.as_ref()).await {
            self.handler.on_io_error(e.into());
            return Err(());
        }

        let replies = self.reader.as_reply_stream();
        tokio::pin!(replies);

        match Self::next_reply(&mut replies).await {
            Ok(reply) => self.handler.on_noop(reply).await,
            Err(e) => {
                self.handler.on_io_error(e);
                Err(())
            }
        }
    }

    pub async fn pre_transaction(&mut self) -> Result<UpgradeTls, ()> {
        let replies = self.reader.as_reply_stream();
        tokio::pin!(replies);

        self.handler.on_connect().await?;
        if self.handler.has_just_connected() {
            let greetings = match Self::next_reply(&mut replies).await {
                Ok(reply) => reply,
                Err(e) => {
                    self.handler.on_io_error(e);
                    return Err(());
                }
            };
            self.handler.on_greetings(greetings).await?;
        }

        let client_name = self.handler.get_client_name();

        // TODO: handle unsupported EHLO (fallback on HELO)
        if let Err(e) = self
            .writer
            .write_all(&format!("EHLO {client_name}\r\n"))
            .await
        {
            self.handler.on_io_error(e.into());
            return Err(());
        }

        let ehlo_reply = match Self::next_reply(&mut replies).await {
            Ok(reply) => reply,
            Err(e) => {
                self.handler.on_io_error(e);
                return Err(());
            }
        };
        self.handler.on_ehlo(ehlo_reply).await
    }

    #[tracing::instrument(skip_all, ret)]
    pub async fn send(&mut self) -> H::Result {
        let Self {
            reader,
            writer,
            handler,
        } = self;

        let replies = reader.as_reply_stream();
        tokio::pin!(replies);

        // TODO: handle the case where DSN is not supported by the remote server, BUT a rcpt required a
        // DSN on success or delayed.
        {
            let envelope = if handler.has_pipelining() {
                Self::send_envelop_pipelining(handler, &mut replies, writer).await
            } else {
                Self::send_envelop_without_pipelining(handler, &mut replies, writer).await
            };
            if envelope == Err(()) {
                return handler.take_result();
            }
        }

        // TODO: handle CHUNKING ?
        if let Err(e) = writer.write_all(Verb::Data.as_ref()).await {
            self.handler.on_io_error(e.into());
            return self.handler.take_result();
        }

        let data_start_reply = match Self::next_reply(&mut replies).await {
            Ok(reply) => reply,
            Err(e) => {
                handler.on_io_error(e);
                return handler.take_result();
            }
        };

        if handler.on_data_start(data_start_reply).await == Err(()) {
            return handler.take_result();
        };

        if let Err(e) = writer.write_all_bytes(&handler.get_message()).await {
            self.handler.on_io_error(e.into());
            return self.handler.take_result();
        }

        if let Err(e) = writer.write_all(".\r\n").await {
            self.handler.on_io_error(e.into());
            return self.handler.take_result();
        }

        let data_end_reply = match Self::next_reply(&mut replies).await {
            Ok(reply) => reply,
            Err(e) => {
                handler.on_io_error(e);
                return handler.take_result();
            }
        };

        let _ = handler.on_data_end(data_end_reply).await;
        handler.take_result()
    }

    pub async fn upgrade_tls(self) -> Result<Self, H::Result> {
        let Self {
            mut reader,
            mut writer,
            mut handler,
        } = self;

        if let Err(e) = writer.write_all(Verb::StartTls.as_ref()).await {
            handler.on_io_error(e.into());
            return Err(handler.take_result());
        }

        let starttls = {
            let replies = reader.as_reply_stream();
            tokio::pin!(replies);

            match Self::next_reply(&mut replies).await {
                Ok(reply) => reply,
                Err(e) => {
                    handler.on_io_error(e);
                    return Err(handler.take_result());
                }
            }
        };

        if starttls.code().value() != 220 {
            return Err(handler.take_result());
        }

        let tcp_stream = {
            let (reader, writer) = (reader.into_inner(), writer.into_inner());
            let (reader, writer) = (Box::into_raw(reader), Box::into_raw(writer));
            // SAFETY: we are converting a pointer to a pointer of the same type
            #[allow(unsafe_code, clippy::cast_ptr_alignment)]
            let (reader, writer) = unsafe {
                (
                    Box::from_raw(reader.cast::<tokio::net::tcp::OwnedReadHalf>()),
                    Box::from_raw(writer.cast::<tokio::net::tcp::OwnedWriteHalf>()),
                )
            };

            reader.reunite(*writer).expect("valid stream/reader pair")
        };

        let sni = handler.get_sni();
        tracing::trace!(
            ?sni,
            peer_addr = ?tcp_stream.peer_addr(),
            "connecting to the remote server"
        );
        let tls_stream = match handler.get_tls_connector().connect(sni, tcp_stream).await {
            Ok(s) => s,
            Err(e) => {
                return Err(handler.on_tls_upgrade_error(e));
            }
        };

        let (reader, writer) = tokio::io::split(tls_stream);

        let (reader, writer) = (
            Box::new(reader) as Box<dyn tokio::io::AsyncRead + Unpin + Send + Sync>,
            Box::new(writer) as Box<dyn tokio::io::AsyncWrite + Unpin + Send + Sync>,
        );

        let (reader, writer) = (Reader::new(reader, true), Writer::new(writer));

        Ok(Self {
            reader,
            writer,
            handler,
        })
    }

    async fn next_reply<S>(reply_stream: &mut S) -> Result<Reply, vsmtp_protocol::Error>
    where
        S: tokio_stream::Stream<Item = Result<Reply, vsmtp_protocol::Error>> + Unpin + Send,
    {
        match tokio_stream::StreamExt::try_next(reply_stream).await {
            Ok(Some(reply)) => Ok(reply),
            Ok(None) => Err(vsmtp_protocol::Error::from(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected eof",
            ))),
            Err(error) => Err(error),
        }
    }

    fn build_mail_from_to_command(
        MailFromProps {
            reverse_path,
            envelop_id,
            ret,
            ..
        }: &MailFromProps,
        has_dsn: bool,
    ) -> String {
        format!(
            "MAIL FROM:<{}>{}\r\n",
            reverse_path
                .as_ref()
                .map_or_else(String::new, ToString::to_string),
            if has_dsn {
                format!(
                    " RET={} {}",
                    match ret {
                        Some(DsnReturn::Full) => "FULL",
                        None | Some(DsnReturn::Headers) => "HDRS",
                    },
                    envelop_id
                        .as_ref()
                        .map_or_else(String::new, |envid| format!("ENVID={envid}"))
                )
            } else {
                String::new()
            }
        )
    }

    fn build_rcpt_to_command(
        Recipient {
            forward_path,
            original_forward_path,
            notify_on,
        }: &Recipient,
        has_dsn: bool,
    ) -> String {
        format!(
            "RCPT TO:<{}>{}\r\n",
            forward_path.0,
            if has_dsn {
                format!(
                    " {} NOTIFY={}",
                    original_forward_path
                        .as_ref()
                        .map_or_else(String::new, |orcpt| format!(
                            "ORCPT={};{}",
                            orcpt.addr_type, orcpt.mailbox
                        )),
                    match notify_on {
                        NotifyOn::Some {
                            success,
                            failure,
                            delay,
                        } => [("SUCCESS", success), ("FAILURE", failure), ("DELAY", delay)]
                            .into_iter()
                            .filter_map(|(value, activated)| activated.then_some(value))
                            .collect::<Vec<_>>()
                            .join(","),
                        NotifyOn::Never => "NEVER".to_owned(),
                    }
                )
            } else {
                String::new()
            }
        )
    }

    async fn send_envelop_pipelining<S, W>(
        handler: &mut H,
        replies: &mut S,
        sink: &mut Writer<W>,
    ) -> Result<(), ()>
    where
        H: SenderHandler + Sync + Send,
        S: tokio_stream::Stream<Item = Result<Reply, vsmtp_protocol::Error>> + Unpin + Send,
        W: tokio::io::AsyncWrite + Unpin + Send + Sync,
    {
        let has_dsn = handler.has_dsn();

        let from = handler.get_mail_from();
        let rcpt = handler.get_rcpt_to();

        let cmd = [
            Self::build_mail_from_to_command(&from, has_dsn),
            rcpt.iter()
                .map(|i| Self::build_rcpt_to_command(i, has_dsn))
                .collect::<String>(),
        ]
        .concat();

        if let Err(e) = sink.write_all(&cmd).await {
            handler.on_io_error(e.into());
            return Err(());
        }

        let mail_from_reply = match Self::next_reply(replies).await {
            Ok(reply) => reply,
            Err(e) => {
                handler.on_io_error(e);
                return Err(());
            }
        };

        handler.on_mail_from(mail_from_reply).await?;

        let mut at_least_one_rcpt_is_valid = false;
        for i in 0..rcpt.len() {
            let rcpt_reply = match Self::next_reply(replies).await {
                Ok(reply) => reply,
                Err(e) => {
                    handler.on_io_error(e);
                    return Err(());
                }
            };

            at_least_one_rcpt_is_valid |= handler
                .on_rcpt_to(rcpt.get(i).unwrap(), rcpt_reply)
                .await
                .is_ok();
        }

        if at_least_one_rcpt_is_valid {
            Ok(())
        } else {
            Err(())
        }
    }

    async fn send_envelop_without_pipelining<S, W>(
        handler: &mut H,
        replies: &mut S,
        sink: &mut Writer<W>,
    ) -> Result<(), ()>
    where
        H: SenderHandler + Sync + Send,
        S: tokio_stream::Stream<Item = Result<Reply, vsmtp_protocol::Error>> + Unpin + Send,
        W: tokio::io::AsyncWrite + Unpin + Send + Sync,
    {
        let has_dsn = handler.has_dsn();

        let from = handler.get_mail_from();
        if let Err(e) = sink
            .write_all(&Self::build_mail_from_to_command(&from, has_dsn))
            .await
        {
            handler.on_io_error(e.into());
            return Err(());
        }

        let mail_from_reply = match Self::next_reply(replies).await {
            Ok(reply) => reply,
            Err(e) => {
                handler.on_io_error(e);
                return Err(());
            }
        };

        handler.on_mail_from(mail_from_reply).await?;

        let rcpt = handler.get_rcpt_to();
        let mut at_least_one_rcpt_is_valid = false;
        for i in rcpt {
            let command = Self::build_rcpt_to_command(&i, has_dsn);
            if let Err(e) = sink.write_all(&command).await {
                handler.on_io_error(e.into());
                return Err(());
            }

            let rcpt_reply = match Self::next_reply(replies).await {
                Ok(reply) => reply,
                Err(e) => {
                    handler.on_io_error(e);
                    return Err(());
                }
            };

            at_least_one_rcpt_is_valid |= handler.on_rcpt_to(&i, rcpt_reply).await.is_ok();
        }

        if at_least_one_rcpt_is_valid {
            Ok(())
        } else {
            Err(())
        }
    }
}
