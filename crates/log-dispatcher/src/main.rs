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

use clap::Parser;
use tokio_stream::StreamExt;
use tracing_amqp::{Event, QUEUE_NAME};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

mod config;

// FIXME: not the right way to dispatch an event
// callsite (line/file/module_path) will be overwritten
#[allow(clippy::cognitive_complexity)]
fn log(level: tracing::Level, target: &str, event: &str) {
    match level {
        tracing::Level::TRACE => {
            tracing::trace!(target: "external", "(from: {}) {}", target, event);
        }
        tracing::Level::DEBUG => {
            tracing::debug!(target: "external", "(from: {}) {}", target, event);
        }
        tracing::Level::INFO => {
            tracing::info!(target: "external", "(from: {}) {}", target, event);
        }
        tracing::Level::WARN => {
            tracing::warn!(target: "external", "(from: {}) {}", target, event);
        }
        tracing::Level::ERROR => {
            tracing::error!(target: "external", "(from: {}) {}", target, event);
        }
    }
}

#[derive(Parser)]
pub struct Args {
    /// Path to the rhai configuration file.
    #[arg(short, long, default_value_t = String::from("/etc/vsmtp/log-dispatcher/conf.d/config.rhai"))]
    pub config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use crate::config::Config;

    let args = <Args as clap::Parser>::parse();
    let config = config::LogDispatcherConfig::from_rhai_file(&args.config)?;

    let mut layers = vec![];
    if let Ok(jaeger) = std::env::var("JAEGER") {
        let jaeger = tracing_opentelemetry::layer().with_tracer(
            opentelemetry_jaeger::new_agent_pipeline()
                .with_service_name("log-dispatcher")
                .with_endpoint(jaeger)
                .install_batch(opentelemetry::runtime::Tokio)
                .unwrap(),
        );
        layers.push(jaeger.boxed());
    }

    if let Ok(loki_url) = std::env::var("LOKI_URL") {
        let (loki_layer, loki_task) = tracing_loki::builder()
            .label("environment", "dev")
            .unwrap()
            .label("host", "localhost")
            .unwrap()
            .build_url(tracing_loki::url::Url::parse(&loki_url).unwrap())
            .unwrap();

        tokio::spawn(loki_task);
        layers.push(loki_layer.boxed());
    }

    let filter =
        tracing_subscriber::filter::Targets::new().with_targets(config.logs().levels.clone());
    // .with_target("external", tracing::Level::WARN);

    let layer = tracing_subscriber::fmt::layer().with_filter(filter);

    layers.push(Box::new(layer));

    tracing_subscriber::registry().with(layers).init();

    let conn = lapin::Connection::connect_with_config(
        &config.broker.uri,
        lapin::ConnectionProperties::default(),
        lapin::tcp::OwnedTLSConfig {
            identity: None,
            cert_chain: config.broker().certificate_chain.clone(),
        },
    )
    .await?;
    let channel = conn.create_channel().await?;
    channel
        .confirm_select(lapin::options::ConfirmSelectOptions::default())
        .await
        .unwrap();
    channel
        .queue_declare(
            QUEUE_NAME,
            lapin::options::QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await
        .unwrap();

    let mut consumer = channel
        .basic_consume(
            QUEUE_NAME,
            "",
            lapin::options::BasicConsumeOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await?;

    tracing::info!("Log dispatcher started");
    while let Some(delivery) = consumer.next().await {
        let delivery = delivery.unwrap();

        match serde_json::from_slice::<Event<'_>>(&delivery.data) {
            Ok(event) => {
                delivery
                    .ack(lapin::options::BasicAckOptions::default())
                    .await
                    .unwrap();

                let field = event.fields.get_key_value("message");
                if let Some(field) = field {
                    log(
                        event.level,
                        event.target,
                        &serde_json::to_string(&field.1).unwrap(),
                    );
                } else {
                    log(
                        event.level,
                        event.target,
                        &serde_json::to_string(&event).unwrap(),
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to deserialized an event: {:?}", e);
            }
        }
    }

    Ok(())
}
