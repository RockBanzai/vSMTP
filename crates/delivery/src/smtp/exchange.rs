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
use futures_util::TryStreamExt;
use vsmtp_common::delivery_attempt::DeliveryAttempt;
use vsmtp_common::transfer_error::Delivery;
use vsmtp_common::{response::Ehlo, stateful_ctx_received::MailFromProps, Recipient};
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

    pub async fn quit(&mut self) -> Result<(), Delivery> {
        self.writer.write_all(Verb::Quit.as_ref()).await?;

        let replies = reply_stream(&mut self.reader);
        tokio::pin!(replies);
        let reply = next_reply(&mut replies).await?;

        self.handler.on_quit(reply).await
    }

    pub async fn noop(&mut self) -> Result<(), Delivery> {
        self.writer.write_all(Verb::Noop.as_ref()).await?;

        let replies = reply_stream(&mut self.reader);
        tokio::pin!(replies);
        let reply = next_reply(&mut replies).await?;

        self.handler.on_noop(reply).await
    }

    pub async fn pre_transaction(&mut self) -> Result<UpgradeTls, Delivery> {
        let replies = reply_stream(&mut self.reader);
        tokio::pin!(replies);

        self.handler.on_connect().await?;
        if self.handler.has_just_connected() {
            self.handler
                .on_greetings(next_reply(&mut replies).await?)
                .await?;
        }

        let client_name = self.handler.get_client_name();

        // TODO: handle unsupported EHLO (fallback on HELO)
        self.writer
            .write_all(&format!("EHLO {client_name}\r\n"))
            .await?;
        self.handler
            .on_ehlo(Ehlo::try_from(next_reply(&mut replies).await?)?)
            .await
    }

    #[tracing::instrument(skip_all, ret)]
    pub async fn send(&mut self) -> DeliveryAttempt {
        let Self {
            reader,
            writer,
            handler,
        } = self;

        let replies = reply_stream(reader);
        tokio::pin!(replies);

        // TODO: handle the case where DSN is not supported by the remote server, BUT a rcpt required a
        // DSN on success or delayed.
        let envelop_result = if handler.has_pipelining() {
            send_envelop_pipelining(handler, &mut replies, writer).await
        } else {
            send_envelop_without_pipelining(handler, &mut replies, writer).await
        };
        if let Err(e) = envelop_result {
            return e;
        }

        // TODO: handle CHUNKING ?
        writer.write_all(Verb::Data.as_ref()).await.unwrap();
        handler
            .on_data_start(next_reply(&mut replies).await.unwrap())
            .await
            .unwrap();

        writer
            .write_all_bytes(&handler.get_message())
            .await
            .unwrap();

        writer.write_all(".\r\n").await.unwrap();
        handler
            .on_data_end(next_reply(&mut replies).await.unwrap())
            .await
            .unwrap();

        handler.get_result()
    }

    pub async fn upgrade_tls(self) -> Result<Self, Delivery> {
        let Self {
            mut reader,
            mut writer,
            handler,
        } = self;

        writer.write_all("STARTTLS\r\n").await?;

        let starttls = {
            let replies = reply_stream(&mut reader);
            tokio::pin!(replies);

            next_reply(&mut replies).await?
        };

        if starttls.code().value() != 220 {
            return Err(Delivery::Tls {
                with_source: Some(format!(
                    "The StartTls command was not successful: {starttls}"
                )),
            });
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
        let tls_stream = handler.get_tls_connector().connect(sni, tcp_stream).await?;

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
}

/// Build a stream of response from the distant server.
fn reply_stream<R>(
    this: &mut Reader<R>,
) -> impl tokio_stream::Stream<Item = Result<Reply, Delivery>> + '_
where
    R: tokio::io::AsyncRead + Unpin + Send,
{
    this.as_reply_stream().map_err(|e| Delivery::Connection {
        with_source: Some(e.to_string()),
    })
}

/// Read the next reply sent by the distant server.
async fn next_reply<S>(reply_stream: &mut S) -> Result<Reply, Delivery>
where
    S: tokio_stream::Stream<Item = Result<Reply, Delivery>> + Unpin + Send,
{
    tokio_stream::StreamExt::try_next(reply_stream)
        .await?
        .ok_or_else(|| Delivery::Connection {
            with_source: Some(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).to_string()),
        })
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

async fn send_envelop_pipelining<H, S, W>(
    handler: &mut H,
    replies: &mut S,
    sink: &mut Writer<W>,
) -> Result<(), DeliveryAttempt>
where
    H: SenderHandler + Sync + Send,
    S: tokio_stream::Stream<Item = Result<Reply, Delivery>> + Unpin + Send,
    W: tokio::io::AsyncWrite + Unpin + Send + Sync,
{
    let has_dsn = handler.has_dsn();

    let from = handler.get_mail_from();
    let rcpt = handler.get_rcpt_to();

    let cmd = [
        build_mail_from_to_command(&from, has_dsn),
        rcpt.iter()
            .map(|i| build_rcpt_to_command(i, has_dsn))
            .collect::<String>(),
    ]
    .concat();

    sink.write_all(&cmd).await.unwrap();

    handler
        .on_mail_from(next_reply(replies).await.unwrap())
        .await
        .unwrap();
    for i in 0..rcpt.len() {
        let rcpt_reply = next_reply(replies).await.unwrap();
        handler
            .on_rcpt_to(rcpt.get(i).unwrap(), rcpt_reply)
            .await
            .unwrap();
    }

    Ok(())
}

async fn send_envelop_without_pipelining<H, S, W>(
    handler: &mut H,
    replies: &mut S,
    sink: &mut Writer<W>,
) -> Result<(), DeliveryAttempt>
where
    H: SenderHandler + Sync + Send,
    S: tokio_stream::Stream<Item = Result<Reply, Delivery>> + Unpin + Send,
    W: tokio::io::AsyncWrite + Unpin + Send + Sync,
{
    let has_dsn = handler.has_dsn();

    let from = handler.get_mail_from();
    sink.write_all(&build_mail_from_to_command(&from, has_dsn))
        .await
        .unwrap();

    handler
        .on_mail_from(next_reply(replies).await.unwrap())
        .await
        .unwrap();

    let rcpt = handler.get_rcpt_to();
    for i in rcpt {
        let command = build_rcpt_to_command(&i, has_dsn);
        sink.write_all(&command).await.unwrap();
        let rcpt_reply = next_reply(replies).await.unwrap();
        handler.on_rcpt_to(&i, rcpt_reply).await.unwrap();
    }

    Ok(())
}
