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

pub mod smtp {
    mod exchange;
    mod handler;
    mod send;

    pub use exchange::Sender;
    pub use handler::{Context, SenderHandler};
    pub use send::send;
}

use std::sync::Arc;
use tokio_stream::StreamExt;
use vsmtp_common::{
    api::{write_to_dead, write_to_deferred, write_to_report_dsn},
    broker::{Exchange, Queue},
    ctx_delivery::CtxDelivery,
    delivery_attempt::{Action, DeliveryAttempt},
    delivery_route::DeliveryRoute,
};
use vsmtp_protocol::NotifyOn;

mod config;
mod frequency;
pub use frequency::Frequency;

pub enum DeliveryOutcome {
    Success,
    Delayed,
    Dead,
}

#[derive(Clone, Copy)]
pub struct ShouldNotify {
    pub on_success: bool,
    pub on_failure: bool,
    pub on_delay: bool,
}

/// Quick check to determine if the delivery method should produce a DSN,
/// based on the delivery attempts, the notification supported by the delivery method and the
/// notification parameters requested for each recipients.
///
/// Return true if only one recipient should produce a DSN.
#[allow(clippy::cognitive_complexity)] // tracing
fn should_produce_dsn(
    attempts: &[DeliveryAttempt],
    ShouldNotify {
        on_success,
        on_failure,
        on_delay,
    }: ShouldNotify,
) -> bool {
    for attempt in attempts {
        for (idx, rcpt) in attempt.recipients().enumerate() {
            match rcpt.notify_on {
                NotifyOn::Never => continue,
                NotifyOn::Some {
                    success,
                    failure,
                    delay,
                } => match attempt.get_action(idx) {
                    Action::Failed { .. } if failure && on_failure => return true,
                    Action::Delayed { .. } if delay && on_delay => return true,
                    Action::Delivered if success && on_success => return true,
                    // TODO:
                    Action::Relayed => todo!(),
                    Action::Expanded => todo!(),
                    _ => continue,
                },
            }
        }
    }

    false
}

#[async_trait::async_trait]
pub trait DeliverySystem: Send + Sync {
    async fn deliver(self: Arc<Self>, ctx: &CtxDelivery) -> Vec<DeliveryAttempt>;

    fn routing_key(&self) -> DeliveryRoute;

    fn get_throttle(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }

    #[must_use]
    fn get_notification_supported() -> ShouldNotify;

    #[tracing::instrument(skip_all, fields(
        uuid = ?ctx.uuid.to_string()[0..8],
        retry = ctx.attempt.len()),
    )]
    async fn do_delivery(self: Arc<Self>, channel: &lapin::Channel, mut ctx: CtxDelivery) {
        let attempts = self.deliver(&ctx).await;

        let should_produce_dsn = should_produce_dsn(&attempts, Self::get_notification_supported());

        ctx.attempt.extend(attempts);
        // FIXME: how to determine the correct threshold?
        // one domain will produce one attempt, meaning mails with multiple domains will inevitably reach this threshold

        let status = if ctx.is_fully_delivered() {
            DeliveryOutcome::Success
        } else if ctx.attempt.len() > 10 {
            DeliveryOutcome::Dead
        } else {
            DeliveryOutcome::Delayed
        };

        if should_produce_dsn {
            tracing::debug!("Message should produce DSN, emitting a report query");
            write_to_report_dsn(channel, ctx.to_json().unwrap()).await;
        } else {
            tracing::debug!("Message should not produce DSN");
        }

        match status {
            DeliveryOutcome::Success => {
                tracing::debug!("Message has been sent successfully, dropping it");
            }
            DeliveryOutcome::Delayed => {
                let delay = ctx.get_delayed_duration();

                tracing::debug!(
                    "Message delivery failed, will retry after {}",
                    humantime::format_duration(delay)
                );

                let payload = ctx.to_json().unwrap();
                let routing_key = ctx.routing_key.to_string();

                write_to_deferred(channel, &routing_key, delay, payload).await;
            }
            DeliveryOutcome::Dead => {
                tracing::debug!("Message delivery failed too many times, putting it in dead queue");

                let payload = ctx.to_json().unwrap();
                write_to_dead(channel, payload).await;
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
async fn init(
    channel: &lapin::Channel,
    system: &impl DeliverySystem,
) -> lapin::Result<tokio_stream::StreamMap<String, lapin::Consumer>> {
    channel
        .exchange_declare(
            Exchange::DelayedDeferred.as_ref(),
            lapin::ExchangeKind::Custom("x-delayed-message".to_string()),
            lapin::options::ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::from(
                std::iter::once(("x-delayed-type".into(), lapin::types::LongString::from("topic").into()))
                    .collect::<std::collections::BTreeMap<lapin::types::ShortString, lapin::types::AMQPValue>>(),
            ),
        )
        .await?;

    channel
        .exchange_declare(
            Exchange::Quarantine.as_ref(),
            lapin::ExchangeKind::Topic,
            lapin::options::ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await?;

    channel
        .queue_declare(
            Queue::Dead.as_ref(),
            lapin::options::QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await?;

    channel
        .queue_declare(
            Queue::DSN.as_ref(),
            lapin::options::QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            Queue::Dead.as_ref(),
            Exchange::Quarantine.as_ref(),
            "dead",
            lapin::options::QueueBindOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await?;

    channel
        .exchange_declare(
            Exchange::Delivery.as_ref(),
            lapin::ExchangeKind::Topic,
            lapin::options::ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await?;

    let mut consumers = vec![];
    let routing_key = system.routing_key().to_string();
    let q_suffix = routing_key.to_string();
    {
        let deferred_q = format!("deferred-{q_suffix}");

        channel
            .queue_declare(
                &deferred_q,
                lapin::options::QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                lapin::types::FieldTable::default(),
            )
            .await?;

        channel
            .queue_bind(
                &deferred_q,
                Exchange::DelayedDeferred.as_ref(),
                &routing_key,
                lapin::options::QueueBindOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await?;

        let consumer = channel
            .basic_consume(
                &deferred_q,
                "",
                lapin::options::BasicConsumeOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await?;

        consumers.push((deferred_q, consumer));
    }

    {
        let delivery_q = format!("{}-{q_suffix}", Exchange::Delivery.as_ref());
        channel
            .queue_declare(
                &delivery_q,
                lapin::options::QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                lapin::types::FieldTable::default(),
            )
            .await?;

        channel
            .queue_bind(
                &delivery_q,
                Exchange::Delivery.as_ref(),
                &routing_key,
                lapin::options::QueueBindOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await?;

        let consumer = channel
            .basic_consume(
                &delivery_q,
                "",
                lapin::options::BasicConsumeOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await?;

        consumers.push((delivery_q, consumer));
    }

    Ok(tokio_stream::StreamMap::from_iter(consumers))
}

pub async fn start_delivery(
    system: std::sync::Arc<impl DeliverySystem + 'static>,
    conn: &lapin::Connection,
) -> Result<(), Box<dyn std::error::Error>> {
    let channel = conn.create_channel().await?;
    channel
        .confirm_select(lapin::options::ConfirmSelectOptions::default())
        .await?;
    channel
        .basic_qos(1, lapin::options::BasicQosOptions::default())
        .await?;

    let consumer = init(&channel, system.as_ref()).await?;
    let consumer = tokio_stream::StreamExt::throttle(consumer, system.get_throttle());

    tokio::pin!(consumer);
    tracing::info!("Delivery service has been started");

    while let Some((_, item)) = consumer.next().await {
        let system = system.clone();
        // FIXME: everything inside the `channel` is an Arc, so we should be able to clone it
        // should be used `conn.create_channel()` instead of `.clone()` ?
        let channel = channel.clone();

        tokio::spawn(async move {
            let item = item.unwrap();
            let lapin::message::Delivery { data, .. } = &item;
            let ctx = match CtxDelivery::from_json(data) {
                Err(e) => {
                    tracing::debug!("handle invaliding payload {}", e);
                    return;
                }
                Ok(ctx) if !system.routing_key().matches(&ctx.routing_key) => {
                    tracing::debug!("handle invaliding routing");
                    return;
                }
                Ok(ctx) => ctx,
            };

            item.ack(lapin::options::BasicAckOptions::default())
                .await
                .expect("ack");

            system.clone().do_delivery(&channel, ctx).await;
        });
    }

    Ok(())
}

pub async fn delivery_main(
    system: std::sync::Arc<impl DeliverySystem + 'static>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::config::Config;
    use tracing_subscriber::prelude::*;

    let args = <config::Args as clap::Parser>::parse();
    let config = config::DeliveryConfig::from_rhai_file(&args.config)?;

    let conn = lapin::Connection::connect_with_config(
        &config.broker().uri,
        lapin::ConnectionProperties::default(),
        lapin::tcp::OwnedTLSConfig {
            identity: None,
            cert_chain: config.broker().certificate_chain.clone(),
        },
    )
    .await?;

    let filter =
        tracing_subscriber::filter::Targets::new().with_targets(config.logs.levels.clone());

    let (layer, task) = tracing_amqp::layer(&conn).await;
    tracing_subscriber::registry()
        .with(layer.with_filter(filter))
        .try_init()
        .unwrap();
    tokio::spawn(task);

    std::panic::set_hook(Box::new(|e| {
        tracing::error!(?e, "panic occurred"); // TODO: check a way to improve formatting from this
    }));
    start_delivery(system, &conn).await
}
