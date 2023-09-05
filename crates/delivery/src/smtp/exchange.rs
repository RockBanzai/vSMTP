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
use futures_util::TryStreamExt;
use vsmtp_common::delivery_attempt::DeliveryAttempt;
use vsmtp_common::transfer_error::Delivery;
use vsmtp_common::{response::Ehlo, stateful_ctx_received::MailFromProps, Recipient};
use vsmtp_protocol::{DsnReturn, NotifyOn, Reader, Reply, Verb, Writer};

pub struct Sender<H: SenderHandler> {
    pub(crate) stream: Reader<Box<dyn tokio::io::AsyncRead + Unpin + Send + Sync>>,
    pub(crate) sink: Writer<Box<dyn tokio::io::AsyncWrite + Unpin + Send + Sync>>,
    pub(crate) handler: H,
}

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

fn mail_from_to_command(
    MailFromProps {
        reverse_path,
        mail_timestamp: _,
        message_uuid: _,
        envelop_id,
        ret,
        spf_mail_from_identity: _,
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
                    .map_or_else(String::new, |envid| format!("ENVID={}", envid))
            )
        } else {
            String::new()
        }
    )
}

fn rcpt_to_command(
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

impl<H> Sender<H>
where
    H: SenderHandler + Sync + Send,
{
    pub fn new(
        stream: Reader<Box<dyn tokio::io::AsyncRead + Unpin + Send + Sync>>,
        sink: Writer<Box<dyn tokio::io::AsyncWrite + Unpin + Send + Sync>>,
        handler: H,
    ) -> Self {
        Self {
            stream,
            sink,
            handler,
        }
    }

    pub fn handler(&mut self) -> &mut H {
        &mut self.handler
    }

    pub async fn pre_transaction(&mut self, context: &mut Context) -> Result<(), Delivery> {
        let replies = reply_stream(&mut self.stream);
        tokio::pin!(replies);

        self.handler.on_connect(context).await?;
        if context.has_value() {
            return Ok(());
        }

        if self.handler.has_just_connected() {
            self.handler
                .on_greetings(next_reply(&mut replies).await?)
                .await?;
        }

        let client_name = self.handler.get_client_name();

        // TODO: handle unsupported EHLO (fallback on HELO)
        self.sink
            .write_all(&format!("EHLO {client_name}\r\n"))
            .await?;
        self.handler
            .on_ehlo(Ehlo::try_from(next_reply(&mut replies).await?)?, context)
            .await?;

        Ok(())
    }

    pub async fn noop(&mut self) -> Result<(), Delivery> {
        self.sink.write_all(Verb::Noop.as_ref()).await?;

        let replies = reply_stream(&mut self.stream);
        tokio::pin!(replies);
        let reply = next_reply(&mut replies).await?;

        self.handler.on_noop(reply).await
    }

    pub async fn quit(&mut self) -> Result<(), Delivery> {
        self.sink.write_all(Verb::Quit.as_ref()).await?;

        let replies = reply_stream(&mut self.stream);
        tokio::pin!(replies);
        let reply = next_reply(&mut replies).await?;

        self.handler.on_quit(reply).await
    }

    async fn send_envelop_pipelining<S>(
        handler: &mut H,
        replies: &mut S,
        sink: &mut Writer<Box<dyn tokio::io::AsyncWrite + Unpin + Send + Sync>>,
    ) -> Result<(), Delivery>
    where
        S: tokio_stream::Stream<Item = Result<Reply, Delivery>> + Unpin + Send,
    {
        let has_dsn = handler.has_dsn();

        let from = handler.get_mail_from();
        let rcpt = handler.get_rcpt_to();

        let cmd = [
            mail_from_to_command(&from, has_dsn),
            rcpt.iter()
                .map(|i| rcpt_to_command(i, has_dsn))
                .collect::<String>(),
        ]
        .concat();

        sink.write_all(&cmd).await?;

        handler.on_mail_from(next_reply(replies).await?).await?;
        for i in 0..rcpt.len() {
            let rcpt_reply = next_reply(replies).await?;
            handler.on_rcpt_to(rcpt.get(i).unwrap(), rcpt_reply).await?;
        }

        Ok(())
    }

    async fn send_envelop_without_pipelining<S>(
        handler: &mut H,
        replies: &mut S,
        sink: &mut Writer<Box<dyn tokio::io::AsyncWrite + Unpin + Send + Sync>>,
    ) -> Result<(), Delivery>
    where
        S: tokio_stream::Stream<Item = Result<Reply, Delivery>> + Unpin + Send,
    {
        let has_dsn = handler.has_dsn();

        let from = handler.get_mail_from();
        sink.write_all(&mail_from_to_command(&from, has_dsn))
            .await?;

        handler.on_mail_from(next_reply(replies).await?).await?;

        let rcpt = handler.get_rcpt_to();
        for i in rcpt {
            let command = rcpt_to_command(&i, has_dsn);
            sink.write_all(&command).await?;
            let rcpt_reply = next_reply(replies).await?;
            handler.on_rcpt_to(&i, rcpt_reply).await?;
        }

        Ok(())
    }

    #[tracing::instrument(skip(self), ret, err)]
    async fn send_inner(&mut self) -> Result<(), Delivery> {
        let Self {
            stream,
            sink,
            handler,
        } = self;

        let replies = reply_stream(stream);
        tokio::pin!(replies);

        // TODO: handle the case where DSN is not supported by the remote server, BUT a rcpt required a
        // DSN on success or delayed.
        if handler.has_pipelining() {
            Self::send_envelop_pipelining(handler, &mut replies, sink).await?;
        } else {
            Self::send_envelop_without_pipelining(handler, &mut replies, sink).await?;
        }

        // TODO: handle CHUNKING ?
        sink.write_all(Verb::Data.as_ref()).await?;
        handler
            .on_data_start(next_reply(&mut replies).await?)
            .await?;

        sink.write_all_bytes(&handler.get_message()).await?;

        sink.write_all(".\r\n").await?;
        handler.on_data_end(next_reply(&mut replies).await?).await?;

        Ok(())
    }

    pub async fn send(&mut self) -> DeliveryAttempt {
        let _ = self.send_inner().await;
        self.handler.get_result()
    }
}
