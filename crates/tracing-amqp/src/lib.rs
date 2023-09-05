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

use tokio_stream::Stream;
use tracing_serde::AsSerde;

#[derive(Default)]
struct Fields {
    fields: serde_json::Map<String, serde_json::Value>,
}

impl Fields {
    fn record_impl(&mut self, field: &tracing::field::Field, value: serde_json::Value) {
        self.fields.insert(field.name().into(), value);
    }

    fn record<T: Into<serde_json::Value>>(&mut self, field: &tracing::field::Field, value: T) {
        self.record_impl(field, value.into());
    }
}

impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.record(field, format!("{value:?}"));
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.record(field, value);
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.record(field, value);
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.record(field, value);
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.record(field, value);
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.record(field, value);
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.record(field, format!("{value}"));
    }
}

pub const QUEUE_NAME: &str = "log";

#[serde_with::serde_as]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Event<'a> {
    #[serde(with = "humantime_serde")]
    pub timestamp: std::time::SystemTime,
    pub name: &'a str,
    pub target: &'a str,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub level: tracing::Level,
    pub module_path: Option<&'a str>,
    pub file: Option<&'a str>,
    pub line: Option<u32>,
    // TODO: kind: Kind,
    #[serde(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub spans: Vec<&'a str>,
}

pub struct Layer {
    sender: tokio::sync::mpsc::Sender<serde_json::Value>,
}

impl<S> tracing_subscriber::Layer<S> for Layer
where
    S: tracing::subscriber::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();
        if extensions.get_mut::<Fields>().is_none() {
            let mut fields = Fields::default();
            attrs.record(&mut fields);
            extensions.insert(fields);
        }
    }

    fn on_record(
        &self,
        id: &tracing::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();
        values.record(extensions.get_mut::<Fields>().expect("unregistered span"));
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let timestamp = std::time::SystemTime::now();

        let spans = ctx
            .current_span()
            .id()
            .and_then(|id| {
                ctx.span_scope(id).map(|scope| {
                    scope.from_root().fold(Vec::new(), |mut spans, span| {
                        spans.push(span.name());
                        spans
                    })
                })
            })
            .unwrap_or_default();

        let fields = serde_json::json!(event.as_serde())
            .as_object()
            .unwrap()
            .clone();

        let event = Event {
            timestamp,
            name: event.metadata().name(),
            target: event.metadata().target(),
            level: *event.metadata().level(),
            module_path: event.metadata().module_path(),
            file: event.metadata().file(),
            line: event.metadata().line(),
            fields,
            spans,
        };
        let json = serde_json::to_value(event);

        if let Ok(json) = json {
            let _: () = self.sender.try_send(json).unwrap_or_default();
        }
    }
}

type TaskOutput = Result<(), Box<dyn std::error::Error>>;
type TaskFuture = Box<dyn std::future::Future<Output = TaskOutput> + Send + 'static>;

pub struct BackgroundTask {
    channel: lapin::Channel,
    receiver: tokio_stream::wrappers::ReceiverStream<serde_json::Value>,
    send_task: Option<std::pin::Pin<TaskFuture>>,
    queue: Vec<serde_json::Value>,
}

impl std::future::Future for BackgroundTask {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut receiver_done = false;

        while let std::task::Poll::Ready(maybe_item) =
            std::pin::Pin::new(&mut self.receiver).poll_next(cx)
        {
            if let Some(item) = maybe_item {
                self.queue.push(item);
            } else {
                receiver_done = true;
            }
        }

        let mut send_task_done;
        loop {
            send_task_done = false;

            if let Some(send_task) = &mut self.send_task {
                match std::pin::Pin::new(send_task).poll(cx) {
                    std::task::Poll::Pending => {}
                    std::task::Poll::Ready(Err(e)) => todo!("{e:?}"),
                    std::task::Poll::Ready(Ok(())) => {
                        self.queue.clear();

                        send_task_done = true;
                    }
                }
            }

            if send_task_done {
                self.send_task = None;
            }
            if self.send_task.is_none() && !self.queue.is_empty() {
                let channel = self.channel.clone();
                let queue = self
                    .queue
                    .iter()
                    .map(|i| serde_json::to_vec(&i).unwrap())
                    .collect::<Vec<_>>();

                self.send_task = Some(Box::pin(async move {
                    let publishes = queue.iter().map(|payload| {
                        channel.basic_publish(
                            "",
                            QUEUE_NAME,
                            lapin::options::BasicPublishOptions::default(),
                            payload,
                            lapin::BasicProperties::default().with_content_type(
                                lapin::types::ShortString::from("application/json"),
                            ),
                        )
                    });
                    let confirms = futures_util::future::try_join_all(publishes).await.unwrap();

                    for confirm in confirms {
                        assert_eq!(
                            confirm.await.unwrap(),
                            lapin::publisher_confirm::Confirmation::Ack(None)
                        );
                    }
                    Ok(())
                }));
            } else {
                break;
            }
        }

        if receiver_done && send_task_done {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    }
}

pub async fn layer(conn: &lapin::Connection) -> (Layer, BackgroundTask) {
    let (tx, rx) = tokio::sync::mpsc::channel(512);

    let layer = Layer { sender: tx };
    let channel = conn.create_channel().await.unwrap();
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

    let task = BackgroundTask {
        channel,
        receiver: rx.into(),
        send_task: None,
        queue: Vec::with_capacity(16),
    };
    (layer, task)
}
