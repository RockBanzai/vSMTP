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
    channel: lapin::Channel,
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

        let mut fields = serde_json::Map::<String, serde_json::Value>::new();
        for field_name in event.metadata().fields() {
            if let Some(value) = json_event.get(field_name.name()) {
                fields.insert(field_name.name().to_string(), value.clone());
            }
        }

        let topic = {
            match fields.remove("topic") {
                // Using `as_str` here because calling `to_string` on a json value
                // messes up the formatting.
                Some(topic) => topic.as_str().unwrap_or("system").to_string(),
                None => "system".to_string(),
            }
        };

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
            topic: topic.clone(),
            hostname: hostname::get().map_or(None, |hostname| hostname.into_string().ok()),
            fields,
            spans,
        };

        if let Ok(payload) = serde_json::to_vec(&event) {
            let publish = self.channel.basic_publish(
                LOG_EXCHANGER_NAME,
                &topic,
                lapin::options::BasicPublishOptions::default(),
                payload.as_slice(),
                lapin::BasicProperties::default()
                    .with_content_type(lapin::types::ShortString::from("application/json")),
            );

            // TODO: use a future instead.
            match tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(publish)
            }) {
                Ok(confirm) => {
                    if tokio::task::block_in_place(move || {
                        tokio::runtime::Handle::current().block_on(confirm)
                    }) != Ok(lapin::publisher_confirm::Confirmation::Ack(None))
                    {
                        eprintln!(
                            "failed to send log message, rabbitmq did not acknowledge the message"
                        );
                    }
                }
                Err(error) => {
                    eprintln!("failed to send log message: {error}");
                }
            }
        }
    }
}

/// Instantiate a amqp tracing layer.
/// This layer send all logs emitted by tracing to a log dispatcher service.
///
/// # Arguments
///
/// * 'conn' - a connection to the server broker
///
/// # Return
///
/// A tracing layer.
///
pub async fn layer(conn: &lapin::Connection) -> Layer {
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

    Layer { channel }
}
