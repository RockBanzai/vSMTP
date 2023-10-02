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

use tracing_amqp::Event;

use chrono::{Datelike, Timelike};

/// Create a message from a log event.
/// If the event contains only a message, it returns the message, otherwise,
/// it uses all custom fields and format them as rust debug print.
///
/// # Arguments:
///
/// * `event` - event received from the log queue.
pub fn get_message(event: &Event) -> Option<String> {
    if event.fields.len() == 1 && event.fields.contains_key("message") {
        return match serde_json::to_string(event.fields.get("message").unwrap()) {
            Ok(msg) => Some(msg.replace('\"', "")),
            Err(_) => None,
        };
    }
    let mut extended_msg = "{".to_string();
    for (name, field) in &event.fields {
        extended_msg.push_str(format!("{}: {}", name, field).as_str());
        extended_msg.push(' ');
    }
    extended_msg.replace_range(extended_msg.len()..extended_msg.len() + 1, "}");
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
pub fn format_level(level: tracing::Level) -> String {
    match level {
        tracing::Level::ERROR => "ERROR",
        tracing::Level::WARN => "WARN",
        tracing::Level::INFO => "INFO",
        tracing::Level::DEBUG => "DEBUG",
        tracing::Level::TRACE => "TRACE",
    }
    .to_string()
}
