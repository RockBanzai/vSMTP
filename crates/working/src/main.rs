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
use vsmtp_working::{
    config::{self, cli::Args},
    rules,
};

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

/// Builder to separate initialization from the main function.
struct Working {
    #[allow(dead_code)]
    config: config::WorkingConfig,
    #[allow(dead_code)]
    conn: lapin::Connection,
    channel: lapin::Channel,
    from_receiver: lapin::Consumer,
    rule_engine_config:
        std::sync::Arc<RuleEngineConfig<StatefulCtxReceived, WorkingStatus, WorkingStage>>,
}

impl Working {
    /// Build the configuration, AMQP connections and rule engine for the service.
    async fn build() -> Result<Self, Box<dyn std::error::Error>> {
        let Args { config } = <Args as clap::Parser>::parse();
        let config = config::WorkingConfig::from_rhai_file(&config).map_err(|error| {
            eprintln!("Failed to boot Working service: {error}");
            error
        })?;
        let conn = config.broker().connect().await?;
        vsmtp_common::init_logs(&conn, config.logs()).await?;

        let channel = conn.create_channel().await?;
        channel
            .confirm_select(lapin::options::ConfirmSelectOptions::default())
            .await?;
        channel
            .basic_qos(1, lapin::options::BasicQosOptions::default())
            .await?;

        let from_receiver = init(&channel).await?;

        let rule_engine_config = std::sync::Arc::new(
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

        Ok(Self {
            config,
            conn,
            channel,
            from_receiver,
            rule_engine_config,
        })
    }

    /// Run the service.
    #[tracing::instrument(name = "working_", skip_all, fields(uuid = ?ctx.mail_from.message_uuid.to_string()[0..8]))]
    async fn run(&mut self, ctx: CtxReceived) {
        let rule_engine = RuleEngine::from_config_with_state(
            self.rule_engine_config.clone(),
            StatefulCtxReceived::Complete(ctx),
        );

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
                    write_to_delivery(
                        &self.channel,
                        &ctx_processed.routing_key.to_string(),
                        payload,
                    )
                    .await;
                }
            }
            WorkingStatus::Fail => unimplemented!(),
            WorkingStatus::Quarantine(name) => {
                tracing::trace!("Putting in quarantine: {}", name);
                let StatefulCtxReceived::Complete(ctx) = rule_engine.take_state() else {
                    unreachable!("the working service always use a complete email")
                };

                let payload = ctx.to_json().unwrap();
                write_to_quarantine(&self.channel, &name, payload).await;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let mut working = match Working::build().await {
        Ok(working) => working,
        Err(error) => {
            eprintln!("Failed to boot Working service: {error}");
            return;
        }
    };

    tracing::info!("Working service is starting");

    while let Some(delivery) = working.from_receiver.next().await {
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

        working.run(ctx).await;
    }
}
