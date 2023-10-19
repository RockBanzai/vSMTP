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

use chrono::{Datelike, Timelike};
use tracing_amqp::Event;

/// Create a message from a log event.
/// If the event contains only a message, it returns the message, otherwise,
/// it uses all custom fields and format them as rust debug print.
///
/// # Arguments:
///
/// * `event` - event received from the log queue.
#[must_use]
pub fn get_message(event: &Event<'_>) -> Option<String> {
    if event.fields.len() == 1 && event.fields.contains_key("message") {
        return serde_json::to_string(event.fields.get("message").unwrap())
            .map_or(None, |msg| Some(msg.replace('\"', "")));
    }
    let mut extended_msg = "{".to_string();
    for (name, field) in &event.fields {
        extended_msg.push_str(format!("{name}: {field}").as_str());
        extended_msg.push(' ');
    }
    extended_msg.replace_range(extended_msg.len() - 1..extended_msg.len(), "}");
    if extended_msg.len() > 2 {
        Some(extended_msg)
    } else {
        None
    }
}

/// Format a timestamp for the console
///
/// # Arguments:
/// * `timestamp` timestamp to format
#[must_use]
pub fn format_timestamp(timestamp: &chrono::DateTime<chrono::Utc>) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        timestamp.year(),
        timestamp.month(),
        timestamp.day(),
        timestamp.hour(),
        timestamp.minute(),
        timestamp.second()
    )
}

/// Format a level for the console
///
/// # Arguments:
/// * `level` level to format
#[must_use]
pub const fn format_level(level: tracing::Level) -> &'static str {
    match level {
        tracing::Level::ERROR => "ERROR",
        tracing::Level::WARN => "WARN",
        tracing::Level::INFO => "INFO",
        tracing::Level::DEBUG => "DEBUG",
        tracing::Level::TRACE => "TRACE",
    }
}
