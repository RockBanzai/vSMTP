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

use crate::broker::{Exchange, Queue};
use lapin::protocol::{AMQPErrorKind, AMQPSoftError};

// NOTE: must put those in a trait, to have a small abstraction layer

pub async fn write_to_working(channel: &lapin::Channel, payload: Vec<u8>) {
    let confirm = channel
        .basic_publish(
            "",
            Queue::ToWorking.as_ref(),
            lapin::options::BasicPublishOptions {
                mandatory: true,
                ..Default::default()
            },
            &payload,
            lapin::BasicProperties::default()
                .with_content_type(lapin::types::ShortString::from("application/json")),
        )
        .await
        .unwrap();

    assert_eq!(
        confirm.await.unwrap(),
        lapin::publisher_confirm::Confirmation::Ack(None)
    );
}

pub async fn write_to_quarantine(channel: &lapin::Channel, quarantine: &str, payload: Vec<u8>) {
    let quarantine_name = format!("rule.{quarantine}");

    let confirm = channel
        .basic_publish(
            Exchange::Quarantine.as_ref(),
            &quarantine_name,
            lapin::options::BasicPublishOptions {
                mandatory: true,
                ..Default::default()
            },
            &payload,
            lapin::BasicProperties::default()
                .with_content_type(lapin::types::ShortString::from("application/json")),
        )
        .await
        .unwrap();

    assert_eq!(
        confirm.await.unwrap(),
        lapin::publisher_confirm::Confirmation::Ack(None)
    );
}

pub async fn write_to_deferred(
    channel: &lapin::Channel,
    routing_key: &str,
    delay: std::time::Duration,
    payload: Vec<u8>,
) {
    let properties = lapin::BasicProperties::default()
        .with_content_type(lapin::types::ShortString::from("application/json"))
        .with_headers(
            std::iter::once((
                "x-delay".into(),
                lapin::types::LongString::from(delay.as_millis().to_string()).into(),
            ))
            .collect::<std::collections::BTreeMap<lapin::types::ShortString, lapin::types::AMQPValue>>()
            .into(),
        );

    let confirm = channel
        .basic_publish(
            Exchange::DelayedDeferred.as_ref(),
            routing_key,
            lapin::options::BasicPublishOptions::default(),
            &payload,
            properties,
        )
        .await
        .unwrap();

    assert_eq!(
        confirm.await.unwrap(),
        lapin::publisher_confirm::Confirmation::Ack(None)
    );
}

pub async fn write_to_report_dsn(channel: &lapin::Channel, payload: Vec<u8>) {
    let confirm = channel
        .basic_publish(
            "",
            Queue::DSN.as_ref(),
            lapin::options::BasicPublishOptions {
                mandatory: true,
                ..Default::default()
            },
            &payload,
            lapin::BasicProperties::default()
                .with_content_type(lapin::types::ShortString::from("application/json")),
        )
        .await
        .unwrap();

    assert_eq!(
        confirm.await.unwrap(),
        lapin::publisher_confirm::Confirmation::Ack(None)
    );
}

pub async fn write_to_dead(channel: &lapin::Channel, payload: Vec<u8>) {
    let confirm = channel
        .basic_publish(
            Exchange::Quarantine.as_ref(),
            "dead",
            lapin::options::BasicPublishOptions {
                mandatory: true,
                ..Default::default()
            },
            &payload,
            lapin::BasicProperties::default()
                .with_content_type(lapin::types::ShortString::from("application/json")),
        )
        .await
        .unwrap();

    assert_eq!(
        confirm.await.unwrap(),
        lapin::publisher_confirm::Confirmation::Ack(None)
    );
}

pub async fn write_to_no_route(channel: &lapin::Channel, payload: Vec<u8>) {
    let confirm = channel
        .basic_publish(
            Exchange::Quarantine.as_ref(),
            Queue::NoRoute.as_ref(),
            lapin::options::BasicPublishOptions {
                mandatory: true,
                ..Default::default()
            },
            &payload,
            lapin::BasicProperties::default()
                .with_content_type(lapin::types::ShortString::from("application/json")),
        )
        .await
        .unwrap();

    assert_eq!(
        confirm.await.unwrap(),
        lapin::publisher_confirm::Confirmation::Ack(None)
    );
}

pub async fn write_to_delivery(channel: &lapin::Channel, routing_key: &str, payload: Vec<u8>) {
    let confirm = channel
        .basic_publish(
            Exchange::Delivery.as_ref(),
            routing_key,
            lapin::options::BasicPublishOptions {
                mandatory: true,
                ..Default::default()
            },
            &payload,
            lapin::BasicProperties::default()
                .with_content_type(lapin::types::ShortString::from("application/json")),
        )
        .await
        .unwrap();

    match confirm.await.unwrap() {
        lapin::publisher_confirm::Confirmation::Ack(None) => {}
        lapin::publisher_confirm::Confirmation::Ack(Some(message)) => {
            if let Some(error) = message.error() {
                match error.kind() {
                    AMQPErrorKind::Soft(AMQPSoftError::NOROUTE) => {
                        write_to_no_route(channel, payload).await;
                    }
                    AMQPErrorKind::Soft(e) => todo!("error not handled {e:?}"),
                    AMQPErrorKind::Hard(e) => todo!("error not handled {e:?}"),
                }
            } else {
                todo!("message was returned, but no error was provided");
            }
        }
        otherwise => todo!("{otherwise:?}"),
    }
}
