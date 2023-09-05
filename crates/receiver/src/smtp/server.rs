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

use crate::smtp::session::{Handler, SaslValidation};
use futures_lite::StreamExt;
use vsmtp_common::uuid;
use vsmtp_protocol::{AcceptArgs, ConnectionKind, ReceiverContext, Reply};

// TODO: how to implement a better virtual host system?
pub struct Server {
    pub socket: std::collections::HashMap<ConnectionKind, Vec<tokio::net::TcpListener>>,
}

impl Server {
    fn listener_to_stream(
        listener: &tokio::net::TcpListener,
    ) -> impl tokio_stream::Stream<
        Item = std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)>,
    > + '_ {
        async_stream::stream! {
            loop {
                yield listener.accept().await;
            }
        }
    }

    fn as_incoming_connection_stream(
        &self,
    ) -> impl futures_lite::Stream<
        Item = (
            (ConnectionKind, std::net::SocketAddr),
            std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)>,
        ),
    > + '_ {
        self.socket
            .iter()
            .flat_map(|(kind, sockets)| {
                sockets.iter().map(|socket| {
                    (
                        (*kind, socket.local_addr().unwrap()),
                        // TODO: can add throttling here
                        Box::pin(Self::listener_to_stream(socket)),
                    )
                })
            })
            .collect::<tokio_stream::StreamMap<_, _>>()
    }

    async fn serve<Fun, Future>(
        on_accept: Fun,
        (server_addr, client_addr, tcp_stream): (
            std::net::SocketAddr,
            std::net::SocketAddr,
            tokio::net::TcpStream,
        ),
        pipelining_support: bool,
    ) where
        Fun: FnOnce(AcceptArgs) -> Future + Send,
        Future: std::future::Future<Output = (Handler, ReceiverContext, Option<Reply>)> + Send,
    {
        let timestamp = time::OffsetDateTime::now_utc();
        let uuid = uuid::Uuid::new_v4();

        let message_stream = vsmtp_protocol::Receiver::<_, SaslValidation, _, _>::new(
            tcp_stream,
            vsmtp_protocol::ConnectionKind::Relay,
            5,
            10,
            20 * 1024 * 1024,
            pipelining_support,
        )
        .into_stream(on_accept, client_addr, server_addr, timestamp, uuid);
        tokio::pin!(message_stream);

        while let Some(item) = message_stream.next().await {
            if item == Ok(()) {
                tracing::info!("Received message");
            } else {
                tracing::warn!("An error terminated the message stream, closing the connection.");
                return;
            }
        }

        tracing::info!("Connection closed cleanly.");
    }

    pub async fn listen<Fun, Future>(&self, on_accept: Fun, pipelining_support: bool)
    where
        Fun: FnOnce(AcceptArgs) -> Future + Send + Clone + 'static,
        Future: std::future::Future<Output = (Handler, ReceiverContext, Option<Reply>)>
            + Send
            + 'static,
    {
        let incoming_connection = self.as_incoming_connection_stream().filter_map(
            |((kind, server_addr), conn)| match conn {
                Err(e) => {
                    tracing::warn!("Error accepting connection on '{kind}/{server_addr}': {e:?}");
                    None
                }
                Ok((tcp_stream, client_addr)) => {
                    tracing::info!("Accepted connection from {client_addr}");
                    Some((server_addr, client_addr, tcp_stream))
                }
            },
        );

        tokio::pin!(incoming_connection);

        // TODO: add max concurrent connections

        while let Some(session) = incoming_connection.next().await {
            tracing::debug!("Serving a new connection");
            tokio::spawn(Self::serve(on_accept.clone(), session, pipelining_support));
        }
    }
}
