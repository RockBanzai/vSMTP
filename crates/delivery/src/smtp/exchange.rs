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

use crate::smtp::handler::UpgradeTls;

use super::{Context, SenderHandler};
use futures_util::TryStreamExt;
use vsmtp_common::delivery_attempt::DeliveryAttempt;
use vsmtp_common::transfer_error::Delivery;
use vsmtp_common::{response::Ehlo, stateful_ctx_received::MailFromProps, Recipient};
use vsmtp_protocol::{DsnReturn, NotifyOn, Reader, Reply, Verb, Writer};

/// Initialize a connection to the distant server and try to send the mail using TLS or an unencrypted connection.
pub async fn deliver_mail<H>(
    mut reader: Reader<tokio::net::tcp::OwnedReadHalf>,
    mut writer: Writer<tokio::net::tcp::OwnedWriteHalf>,
    handler: &mut H,
    context: &mut Context,
) -> Result<DeliveryAttempt, Delivery>
where
    H: SenderHandler + Sync + Send,
{
    let ehlo = {
        handler.on_connect(context).await?;
        if context.has_value() {
            return send(reader, writer, handler).await;
        }

        let replies = reply_stream(&mut reader);
        tokio::pin!(replies);

        if handler.has_just_connected() {
            handler
                .on_greetings(next_reply(&mut replies).await?)
                .await?;
        }

        let client_name = handler.get_client_name();

        // TODO: handle unsupported EHLO (fallback on HELO)
        writer.write_all(&format!("EHLO {client_name}\r\n")).await?;

        Ehlo::try_from(next_reply(&mut replies).await?)
    }?;

    if let UpgradeTls::Yes = handler.on_ehlo(ehlo, context).await? {
        let (reader, writer) = tls(reader, writer, handler).await?;
        send(reader, writer, handler).await
    } else {
        send(reader, writer, handler).await
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
                    .map_or_else(String::new, |envid| format!("ENVID={}", envid))
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

/// Create a sender that uses tls.
async fn tls<H>(
    mut reader: Reader<tokio::net::tcp::OwnedReadHalf>,
    mut writer: Writer<tokio::net::tcp::OwnedWriteHalf>,
    handler: &H,
) -> Result<
    (
        Reader<
            tokio::io::ReadHalf<
                vsmtp_protocol::tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
            >,
        >,
        Writer<
            tokio::io::WriteHalf<
                vsmtp_protocol::tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
            >,
        >,
    ),
    Delivery,
>
where
    H: SenderHandler + Sync + Send,
{
    writer.write_all("StartTls\r\n").await?;

    let starttls = {
        let replies = reply_stream(&mut reader);
        tokio::pin!(replies);

        next_reply(&mut replies).await?
    };

    if starttls.code().value() == 220 {
        let full_stream = writer
            .into_inner()
            .reunite(reader.into_inner())
            .expect("valid stream/sink pair");

        let sni = handler.get_sni();

        handler
            .get_tls_connector()
            .connect(sni, full_stream)
            .await
            .map(|stream| {
                let (reader, writer) = tokio::io::split(stream);
                let reader = Reader::new(reader, true);
                let writer = Writer::new(writer);

                (reader, writer)
            })
            .map_err(|error| error.into())
    } else {
        Err(Delivery::Tls {
            with_source: Some(format!(
                "The StartTls command was not successful: {starttls}"
            )),
        })
    }
}

#[tracing::instrument(skip_all, ret, err)]
pub async fn send<H, R, W>(
    mut reader: Reader<R>,
    mut writer: Writer<W>,
    handler: &mut H,
) -> Result<DeliveryAttempt, Delivery>
where
    H: SenderHandler + Sync + Send,
    R: tokio::io::AsyncRead + Unpin + Send + Sync,
    W: tokio::io::AsyncWrite + Unpin + Send + Sync,
{
    let replies = reply_stream(&mut reader);
    tokio::pin!(replies);

    // TODO: handle the case where DSN is not supported by the remote server, BUT a rcpt required a
    // DSN on success or delayed.
    if handler.has_pipelining() {
        send_envelop_pipelining(handler, &mut replies, &mut writer).await?;
    } else {
        send_envelop_without_pipelining(handler, &mut replies, &mut writer).await?;
    }

    // TODO: handle CHUNKING ?
    writer.write_all(Verb::Data.as_ref()).await?;
    handler
        .on_data_start(next_reply(&mut replies).await?)
        .await?;

    writer.write_all_bytes(&handler.get_message()).await?;

    writer.write_all(".\r\n").await?;
    handler.on_data_end(next_reply(&mut replies).await?).await?;

    Ok(handler.get_result())
}

async fn send_envelop_pipelining<H, S, W>(
    handler: &mut H,
    replies: &mut S,
    sink: &mut Writer<W>,
) -> Result<(), Delivery>
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

    sink.write_all(&cmd).await?;

    handler.on_mail_from(next_reply(replies).await?).await?;
    for i in 0..rcpt.len() {
        let rcpt_reply = next_reply(replies).await?;
        handler.on_rcpt_to(rcpt.get(i).unwrap(), rcpt_reply).await?;
    }

    Ok(())
}

async fn send_envelop_without_pipelining<H, S, W>(
    handler: &mut H,
    replies: &mut S,
    sink: &mut Writer<W>,
) -> Result<(), Delivery>
where
    H: SenderHandler + Sync + Send,
    S: tokio_stream::Stream<Item = Result<Reply, Delivery>> + Unpin + Send,
    W: tokio::io::AsyncWrite + Unpin + Send + Sync,
{
    let has_dsn = handler.has_dsn();

    let from = handler.get_mail_from();
    sink.write_all(&build_mail_from_to_command(&from, has_dsn))
        .await?;

    handler.on_mail_from(next_reply(replies).await?).await?;

    let rcpt = handler.get_rcpt_to();
    for i in rcpt {
        let command = build_rcpt_to_command(&i, has_dsn);
        sink.write_all(&command).await?;
        let rcpt_reply = next_reply(replies).await?;
        handler.on_rcpt_to(&i, rcpt_reply).await?;
    }

    Ok(())
}
