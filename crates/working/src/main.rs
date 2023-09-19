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

use futures_lite::stream::StreamExt;
use rules::{stage::WorkingStage, status::WorkingStatus};
use vsmtp_common::{
    api::{write_to_delivery, write_to_quarantine},
    broker::{Exchange, Queue},
    ctx_received::CtxReceived,
    stateful_ctx_received::StatefulCtxReceived,
};
use vsmtp_config::Config;
use vsmtp_rule_engine::{
    api::{server_auth, utils_modules},
    rhai, RuleEngine, RuleEngineConfig, RuleEngineConfigBuilder,
};
use vsmtp_working::{config, rules};

async fn init(channel: &lapin::Channel) -> lapin::Result<lapin::Consumer> {
    let _to_working = channel
        .queue_declare(
            Queue::ToWorking.as_ref(),
            lapin::options::QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await?;

    let _no_route_fallback = channel
        .queue_declare(
            Queue::NoRoute.as_ref(),
            lapin::options::QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
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
        .queue_bind(
            Queue::NoRoute.as_ref(),
            Exchange::Quarantine.as_ref(),
            Queue::NoRoute.as_ref(),
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

    let consumer = channel
        .basic_consume(
            Queue::ToWorking.as_ref(),
            "",
            lapin::options::BasicConsumeOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await?;

    Ok(consumer)
}

#[tracing::instrument(name = "working_", skip_all, fields(
    uuid = ?ctx.mail_from.message_uuid.to_string()[0..8],
))]
async fn working(
    channel: &lapin::Channel,
    rule_engine_config: std::sync::Arc<
        RuleEngineConfig<StatefulCtxReceived, WorkingStatus, WorkingStage>,
    >,
    ctx: CtxReceived,
) {
    let rule_engine =
        RuleEngine::from_config_with_state(rule_engine_config, StatefulCtxReceived::Complete(ctx));

    match rule_engine.run(&WorkingStage::PostQueue) {
        WorkingStatus::Next | WorkingStatus::Success => {
            let StatefulCtxReceived::Complete(CtxReceived {
                connect: _,
                helo: _,
                mail_from,
                rcpt_to,
                mail,
                complete: _,
            }) = rule_engine.take_state()
            else {
                unreachable!("the working service always use a complete email")
            };

            let deliveries = rcpt_to
                .recipient
                .into_iter()
                .filter(|(_, v)| !v.is_empty())
                .map(|(route, recipient)| {
                    vsmtp_common::ctx_delivery::CtxDelivery::new(
                        route,
                        mail_from.clone(),
                        recipient,
                        mail.clone(),
                    )
                })
                .collect::<Vec<_>>();

            for ctx_processed in deliveries {
                let payload = ctx_processed.to_json().unwrap();
                tracing::warn!("Sending to delivery at: {}", ctx_processed.routing_key);
                write_to_delivery(channel, &ctx_processed.routing_key.to_string(), payload).await;
            }
        }
        WorkingStatus::Fail => unimplemented!(),
        WorkingStatus::Quarantine(name) => {
            tracing::trace!("Putting in quarantine: {}", name);
            let StatefulCtxReceived::Complete(ctx) = rule_engine.take_state() else {
                unreachable!("the working service always use a complete email")
            };

            let payload = ctx.to_json().unwrap();
            write_to_quarantine(channel, &name, payload).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::prelude::*;

    let args = <config::cli::Args as clap::Parser>::parse();
    let config = config::WorkingConfig::from_rhai_file(&args.config).map_err(|error| {
        eprintln!("Failed to boot Working service: {error}");
        error
    })?;

    let conn = lapin::Connection::connect_with_config(
        &config.broker.uri,
        lapin::ConnectionProperties::default(),
        lapin::tcp::OwnedTLSConfig {
            identity: None,
            cert_chain: config.broker.certificate_chain.clone(),
        },
    )
    .await?;

    let filter = tracing_subscriber::filter::Targets::new()
        .with_targets(config.logs.levels.clone())
        .with_default(config.logs().default_level);

    let (layer, task) = tracing_amqp::layer(&conn).await;
    tracing_subscriber::registry()
        .with(layer.with_filter(filter))
        .try_init()
        .unwrap();
    tokio::spawn(task);

    std::panic::set_hook(Box::new(|e| {
        tracing::error!(?e, "Panic occurred");
    }));

    let channel = conn.create_channel().await?;
    channel
        .confirm_select(lapin::options::ConfirmSelectOptions::default())
        .await?;
    channel
        .basic_qos(1, lapin::options::BasicQosOptions::default())
        .await?;

    let mut from_receiver = init(&channel).await?;

    let rule_engine_config =
        std::sync::Arc::new(
            RuleEngineConfigBuilder::default()
                .with_configuration(&config)?
                .with_default_module_resolvers(config.scripts.path.parent().ok_or_else(|| {
                    format!("Invalid script path: {}", config.scripts.path.display())
                })?)
                .with_standard_global_modules()
                .with_smtp_modules()
                .with_static_modules(
                    std::iter::once((
                        "status".to_string(),
                        rhai::exported_module!(rules::api::status).into(),
                    ))
                    .chain(server_auth())
                    .chain(utils_modules())
                    .chain([
                        vsmtp_rhai_utils::time(),
                        vsmtp_rhai_utils::env(),
                        vsmtp_rhai_utils::process(),
                        vsmtp_rhai_utils::crypto(),
                    ]),
                )
                .with_script_at(
                    &config.scripts.path,
                    "/etc/vsmtp/working/conf.d/config.rhai",
                )?
                .build(),
        );

    tracing::info!("Working service is starting");
    while let Some(delivery) = from_receiver.next().await {
        let delivery = delivery.expect("error in consumer");

        let lapin::message::Delivery { data, .. } = &delivery;
        let ctx = match CtxReceived::from_json(data) {
            Ok(ctx) => ctx,
            Err(e) => {
                todo!("handle invaliding payload {e:?}");
            }
        };

        delivery
            .ack(lapin::options::BasicAckOptions::default())
            .await
            .expect("ack");

        working(&channel, rule_engine_config.clone(), ctx).await;
    }

    Ok(())
}
