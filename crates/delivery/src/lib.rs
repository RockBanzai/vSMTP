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

    pub use exchange::Sender;
    pub use handler::{SenderHandler, UpgradeTls};
}

mod send;
pub use send::send;

use std::sync::Arc;
use tokio_stream::StreamExt;
use vsmtp_common::{
    api::{write_to_dead, write_to_deferred, write_to_report_dsn},
    broker::{Exchange, Queue},
    ctx::Ctx,
    ctx_delivery::CtxDelivery,
    delivery_attempt::{Action, DeliveryAttempt, ShouldNotify},
    delivery_route::DeliveryRoute,
    Recipient,
};
use vsmtp_config::Config;
use vsmtp_protocol::NotifyOn;

mod frequency;
pub use frequency::Frequency;
mod tls;
pub use tls::{Requirement, Tls};

pub enum DeliveryOutcome {
    Success,
    Delayed,
    Dead,
}

/// Quick check to determine if the delivery method should produce a DSN,
/// based on the delivery attempts, the notification supported by the delivery method and the
/// notification parameters requested for each recipients.
///
/// Return true if only one recipient should produce a DSN.
#[allow(clippy::cognitive_complexity)] // tracing
fn should_produce_dsn(attempts: &[DeliveryAttempt], recipients: &[Recipient]) -> bool {
    for attempt in attempts {
        for (idx, rcpt) in attempt.recipients().enumerate() {
            let Some(rcpt) = recipients.iter().find(|i| i.forward_path.0 == rcpt.0) else {
                continue;
            };

            match rcpt.notify_on {
                NotifyOn::Never => continue,
                NotifyOn::Some {
                    success,
                    failure,
                    delay,
                } => match attempt.get_action(idx) {
                    Action::Failed { .. }
                        if failure && attempt.should_notify_on(ShouldNotify::Failure) =>
                    {
                        return true
                    }
                    Action::Delayed { .. }
                        if delay && attempt.should_notify_on(ShouldNotify::Delay) =>
                    {
                        return true
                    }
                    Action::Delivered
                        if success && attempt.should_notify_on(ShouldNotify::Success) =>
                    {
                        return true
                    }
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
    fn name(&self) -> &str;

    async fn deliver(self: Arc<Self>, ctx: &CtxDelivery) -> Vec<DeliveryAttempt>;

    fn routing_key(&self) -> DeliveryRoute;

    fn get_throttle(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }

    #[tracing::instrument(skip_all, fields(
        uuid = ?ctx.metadata.uuid.to_string()[0..8],
        retry = ctx.metadata.attempt.len()),
    )]
    async fn do_delivery(self: Arc<Self>, channel: &lapin::Channel, mut ctx: Ctx<CtxDelivery>) {
        let attempts = self.deliver(&ctx.metadata).await;
        ctx.metadata.last_deliveries = attempts;

        let should_produce_dsn =
            should_produce_dsn(&ctx.metadata.last_deliveries, &ctx.metadata.rcpt_to);
        if should_produce_dsn {
            tracing::debug!("Message should produce DSN, emitting a report request");
            write_to_report_dsn(channel, ctx.to_json().unwrap()).await;
        } else {
            tracing::debug!("Message should not produce DSN");
        }
        let last_deliveries = std::mem::take(&mut ctx.metadata.last_deliveries);
        ctx.metadata.attempt.extend(last_deliveries);

        // FIXME: how to determine the correct threshold?
        // one domain will produce one attempt, meaning mails with multiple domains will inevitably reach this threshold
        let status = if ctx.metadata.is_fully_delivered() {
            DeliveryOutcome::Success
        } else if ctx.metadata.attempt.len() > 10 {
            DeliveryOutcome::Dead
        } else {
            DeliveryOutcome::Delayed
        };

        match status {
            DeliveryOutcome::Success => {
                tracing::debug!("Message has been sent successfully, dropping it");
            }
            DeliveryOutcome::Delayed => {
                let delay = ctx.metadata.get_delayed_duration();

                tracing::debug!(
                    "Message delivery failed, will retry after {}",
                    humantime::format_duration(delay)
                );

                let payload = ctx.to_json().unwrap();
                let routing_key = ctx.metadata.routing_key.to_string();

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

        tracing::debug!("Starting consumer for {}", deferred_q);
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

        tracing::debug!("Starting consumer for {}", delivery_q);
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
        let channel = channel.clone();

        tokio::spawn(async move {
            let item = item.unwrap();
            let lapin::message::Delivery { data, .. } = &item;
            let ctx = match Ctx::<CtxDelivery>::from_json(data) {
                Err(e) => {
                    tracing::debug!("handle invaliding payload {}", e);
                    return;
                }
                Ok(ctx) if !system.routing_key().matches(&ctx.metadata.routing_key) => {
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
    system: std::sync::Arc<impl DeliverySystem + Config + 'static>,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = system.broker().connect().await?;
    vsmtp_common::init_logs(&conn, system.logs(), system.name()).await?;
    start_delivery(system, &conn).await
}
