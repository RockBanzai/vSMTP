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

use crate::{Topic, LOG_EXCHANGER_NAME};
use futures_util::Stream;
use std::task::Poll;

type DispatcherOutput = Result<(), Box<dyn std::error::Error>>;
type DispatcherFuture = Box<dyn std::future::Future<Output = DispatcherOutput> + Send + 'static>;

pub struct Dispatcher {
    pub(crate) channel: lapin::Channel,
    pub(crate) receiver: tokio_stream::wrappers::ReceiverStream<(Topic, Vec<u8>)>,
    pub(crate) send_task: Option<std::pin::Pin<DispatcherFuture>>,
    pub(crate) queue: Vec<(Topic, Vec<u8>)>,
}

async fn make_sending_task(
    channel: lapin::Channel,
    queue: Vec<(Topic, Vec<u8>)>,
) -> DispatcherOutput {
    let publishes = queue.iter().map(|(topic, payload)| {
        channel.basic_publish(
            LOG_EXCHANGER_NAME,
            topic,
            lapin::options::BasicPublishOptions::default(),
            payload,
            lapin::BasicProperties::default()
                .with_content_type(lapin::types::ShortString::from("application/json")),
        )
    });

    let publishes = futures_util::future::try_join_all(publishes)
        .await?
        .into_iter();
    let confirms = futures_util::future::try_join_all(publishes).await?;

    for confirm in confirms {
        if !matches!(confirm, lapin::publisher_confirm::Confirmation::Ack(None)) {
            eprintln!("message sent to log dispatcher was not acknowledged");
        }
    }

    Ok(())
}

impl std::future::Future for Dispatcher {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let mut receiver_done = false;

        while let Poll::Ready(maybe_item) = std::pin::Pin::new(&mut self.receiver).poll_next(cx) {
            if let Some(item) = maybe_item {
                self.queue.push(item);
            } else {
                receiver_done = true;
                break;
            }
        }

        let mut send_task_done = false;
        loop {
            if let Some(send_task) = &mut self.send_task {
                match std::pin::Pin::new(send_task).poll(cx) {
                    Poll::Pending => {}
                    Poll::Ready(Err(error)) => {
                        eprintln!("failed to send logs to log dispatcher: {error}")
                    }
                    Poll::Ready(Ok(())) => {
                        send_task_done = true;
                        self.send_task = None;
                    }
                }
            }

            if self.send_task.is_none() && !self.queue.is_empty() {
                self.send_task = Some(Box::pin(make_sending_task(
                    self.channel.clone(),
                    std::mem::take(&mut self.queue),
                )));
            } else {
                break;
            }
        }

        if receiver_done && send_task_done {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
