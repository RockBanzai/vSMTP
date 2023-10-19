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

use std::collections::HashMap;

use crate::config::Config;
use clap::Parser;
use config::LogFormat;
use tokio_stream::{StreamExt, StreamMap};
use tracing_amqp::{Event, LOG_EXCHANGER_NAME};
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Layer,
};

mod config;
mod formatter;
mod logger;

const FORMATTERS_RFC3164: formatter::Rfc3164 = formatter::Rfc3164;
const FORMATTERS_RFC5424: formatter::Rfc5424 = formatter::Rfc5424;

#[derive(Parser)]
pub struct Args {
    /// Path to the rhai configuration file.
    #[arg(short, long, default_value_t = String::from("/etc/vsmtp/log-dispatcher/conf.d/config.rhai"))]
    pub config: String,
}

/// Error which can happens in the log dispatcher
#[derive(thiserror::Error, Debug)]
pub enum LogDispatcherError {
    #[error("wrong configuration for log-dispatcher: {0}")]
    IncompatibleParameter(String),
}

/// Get a pointer on a formatter based on a type of formatter
///
/// # Arguments:
/// * `format` the type of formatter
fn instantiate_formatter(format: LogFormat) -> Box<dyn formatter::Formatter> {
    match format {
        LogFormat::Rfc3164 => Box::new(FORMATTERS_RFC3164),
        LogFormat::RFC5424 => Box::new(FORMATTERS_RFC5424),
    }
}

/// Instantiate a logger based on its configuration
///
/// # Arguments:
/// * `config` configuration of a logger instance
fn instantiate_logger(config: config::LogInstanceType) -> Box<dyn logger::Logger> {
    match config {
        config::LogInstanceType::Console { formatter } => {
            let mut logger = logger::Console::default();
            if let Some(formatter) = formatter {
                logger.set_formatter(instantiate_formatter(formatter));
            }
            Box::new(logger)
        }
        config::LogInstanceType::File {
            folder,
            rotation,
            file_prefix,
        } => Box::new(logger::File::new(rotation, folder, file_prefix)),
        config::LogInstanceType::Syslog {
            formatter,
            protocol,
            address,
        } => Box::new(logger::Syslog::new(
            protocol,
            address,
            instantiate_formatter(formatter),
        )),
        config::LogInstanceType::Journald => Box::new(logger::Journald::new()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = <Args as clap::Parser>::parse();
    let config = config::LogDispatcherConfig::from_rhai_file(&args.config)?;
    // for internal logs
    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("vsmtp_log_dispatcher", tracing::Level::TRACE);
    let layer = tracing_subscriber::fmt::layer().with_filter(filter);
    tracing_subscriber::registry().with(layer).init();

    let conn = config.broker.connect().await?;

    let mut consumers = StreamMap::new(); // rabbitmq consumers
    let mut loggers = HashMap::<String, Vec<Box<dyn logger::Logger>>>::new();
    for logger in config.loggers {
        if !loggers.contains_key(&logger.topic) {
            loggers.insert(logger.topic.clone(), Vec::new());
        }
        loggers
            .get_mut(&logger.topic)
            .unwrap()
            .push(instantiate_logger(logger.config));
        if consumers.contains_key(&logger.topic) {
            continue;
        }
        let channel = conn.create_channel().await?;
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

        let queue = channel
            .queue_declare(
                format!("log-dispatcher-{}", logger.topic).as_str(),
                lapin::options::QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                lapin::types::FieldTable::default(),
            )
            .await
            .unwrap();

        channel
            .queue_bind(
                queue.name().as_str(),
                LOG_EXCHANGER_NAME,
                &logger.topic,
                lapin::options::QueueBindOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .unwrap();

        consumers.insert(
            logger.topic,
            channel
                .basic_consume(
                    queue.name().as_str(),
                    LOG_EXCHANGER_NAME,
                    lapin::options::BasicConsumeOptions::default(),
                    lapin::types::FieldTable::default(),
                )
                .await?,
        );
    }

    tracing::info!("Log dispatcher has started");
    while let Some((topic, delivery)) = consumers.next().await {
        if loggers.contains_key(&topic) {
            let delivery = delivery.unwrap();
            match serde_json::from_slice::<Event<'_>>(&delivery.data) {
                Ok(event) => {
                    delivery
                        .ack(lapin::options::BasicAckOptions::default())
                        .await
                        .unwrap();
                    if let Some(loggers) = loggers.get_mut(&topic) {
                        for logger in loggers {
                            logger.log(&event);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialized a log message: {}", e);
                }
            }
        }
    }
    tracing::warn!("Log dispatcher has stopped");

    Ok(())
}
