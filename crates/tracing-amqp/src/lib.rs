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

pub mod dispatcher;
pub mod layer;
pub use dispatcher::Dispatcher;
pub use layer::Event;
pub use layer::Layer;

pub const LOG_EXCHANGER_NAME: &str = "log";

type Topic = String;

/// Instantiate a amqp tracing layer.
/// This layer send all logs emitted by tracing to a log dispatcher service.
///
/// # Arguments
///
/// * 'conn'         - a connection to the server broker
/// * 'service_name' - the name/id/hostname of the service which will send logs.
///
/// # Return
///
/// A tracing layer.
///
pub async fn layer(conn: &lapin::Connection, service_name: &str) -> (Layer, Dispatcher) {
    let (tx, rx) = tokio::sync::mpsc::channel(512);

    let channel = conn.create_channel().await.unwrap();
    channel
        .confirm_select(lapin::options::ConfirmSelectOptions::default())
        .await
        .unwrap();
    channel
        .exchange_declare(
            LOG_EXCHANGER_NAME,
            lapin::ExchangeKind::Topic,
            lapin::options::ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await
        .unwrap();

    (
        Layer {
            sender: tx,
            service_name: format!(
                "{}.{}",
                service_name,
                hostname::get()
                    .ok()
                    .as_ref()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
            ),
        },
        Dispatcher {
            channel,
            receiver: rx.into(),
            send_task: None,
            queue: Vec::with_capacity(16),
        },
    )
}
