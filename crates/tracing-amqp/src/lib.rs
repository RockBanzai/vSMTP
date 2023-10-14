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
pub const LOG_EXCHANGER_NAME: &str = "log";

#[serde_with::serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Event<'a> {
    /// Timestamp of the event
    #[serde(with = "humantime_serde")]
    pub timestamp: std::time::SystemTime,
    /// Name of the event
    pub name: &'a str,
    /// Where the log come from
    pub target: &'a str,
    /// Level of the event (from error to trace)
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub level: tracing::Level,
    /// Path of where the log come from
    pub module_path: Option<&'a str>,
    /// File of where the log come from
    pub file: Option<&'a str>,
    /// Line of where the log come from
    pub line: Option<u32>,
    /// Kind of event following tracing kind (1 = event, 2 = span, 4 = hint)
    pub kind: u8,
    /// Which topic the event will be stored / can be retrieve from on the logging queue
    pub topic: String,
    /// hostname of the machine emitting the log
    pub hostname: Option<String>,
    /// list of custom fields in the event
    #[serde(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
    /// span(s) from which the event come from
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

        let json_event = serde_json::json!(event.as_serde())
            .as_object()
            .unwrap()
            .clone();

        let mut fields: serde_json::Map<String, serde_json::Value> =
            serde_json::Map::<String, serde_json::Value>::new();
        for field_name in event.metadata().fields() {
            let filed_value = json_event.get(field_name.name());
            if let Some(value) = filed_value {
                fields.insert(field_name.name().to_string(), value.clone());
            }
        }
        if let Some(topic) = fields.remove("target_topic") {
            fields.insert("topic".to_string(), topic);
        }
        if !fields.contains_key("topic") {
            fields.insert("topic".to_string(), serde_json::json!("system"));
        }

        // unfortunately, there is no kind getter
        let kind: u8 = if event.metadata().is_event() {
            1 // tracing::metadata::Kind::EVENT
        } else if event.metadata().is_span() {
            2 // tracing::metadata::Kind::SPAN
        } else {
            4 // tracing::metadata::Kind::HINT
        };

        let event = Event {
            timestamp,
            name: event.metadata().name(),
            target: event.metadata().target(),
            level: *event.metadata().level(),
            module_path: event.metadata().module_path(),
            file: event.metadata().file(),
            line: event.metadata().line(),
            kind,
            topic: "system".to_string(), // this string will be replaced on serialization if needed
            hostname: {
                if let Ok(hostname) = hostname::get() {
                    if let Ok(hostname) = hostname.into_string() {
                        Some(hostname)
                    } else {
                        None
                    }
                } else {
                    None
                }
            },
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

        // the topic the event will be send on
        let mut topic = "system".to_string();
        while let std::task::Poll::Ready(maybe_item) =
            std::pin::Pin::new(&mut self.receiver).poll_next(cx)
        {
            if let Some(item) = maybe_item {
                if let Some(event_topic) = item.get("topic") {
                    if let Some(event_topic_str) = event_topic.as_str() {
                        topic = event_topic_str.to_string();
                    }
                }
                self.queue.push(item);
            } else {
                receiver_done = true;
            }
        }

        let mut send_task_done;
        loop {
            let cloned_topic = topic.clone();
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
                            LOG_EXCHANGER_NAME,
                            &cloned_topic,
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

/// Instantiate a amqp tracing layer.
/// This layer send received log events to a "log" exchanger
/// The returns values are the Layer in itself and a background task to run in a `tokio::spawn`
///
/// # Arguments
///
/// * 'conn' - a connection to the server broker
pub async fn layer(conn: &lapin::Connection) -> (Layer, BackgroundTask) {
    let (tx, rx) = tokio::sync::mpsc::channel(512);

    let layer = Layer { sender: tx };
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

    let task = BackgroundTask {
        channel,
        receiver: rx.into(),
        send_task: None,
        queue: Vec::with_capacity(16),
    };
    (layer, task)
}
