use chrono::{DateTime, Datelike, Timelike, Utc};
use thiserror::Error;
use tracing::Level;
use tracing_amqp::Event;

pub trait Formatter {
    fn format(&self, event: &Event) -> Result<String, FormatterError>;
}

pub fn extract_first_span(target: &String) -> String {
    target[0..target.find(':').unwrap_or(target.len())].to_string()
}

/// Error which can happens in a formatter
#[derive(Error, Debug)]
pub enum FormatterError {
    /// No message can be created by the received log event.
    #[error("Their is not message to log")]
    NoMessage,
    /// The received event is not a log event.
    #[error("This event is not a log but a span")]
    NotAnEvent,
}

/// Convert a tracing level to a syslog level.
pub fn level_to_syslog_level(level: &Level) -> u8 {
    match *level {
        Level::ERROR => 3,
        Level::WARN => 4,
        Level::INFO => 6,
        Level::DEBUG => 7,
        Level::TRACE => 7,
    }
}

/// Syslog facility
/// Note: 2 is mail system messages * 8 for alignment
const DEFAULT_SYSLOG_FACILITY: u8 = 2 * 8;

/// Content of a rfc 5424 compliant message.
struct Rfc5424Msg {
    /// Syslog facility
    pub facility: u8,
    /// Gravity of the message (=level)
    pub gravity: u8,
    /// Timestamp of the message.
    pub timestamp: DateTime<Utc>,
    /// Hostname of the log emitter.
    pub hostname: String,
    /// Name of the app which emit the log
    pub app_name: String,
    /// Content of the log message
    pub content: String,
    /// procID
    pub proc_id: String,
}

impl ToString for Rfc5424Msg {
    fn to_string(&self) -> String {
        let msg = format!(
            "<{}>{} {} {} {} {} {} {}",
            self.facility | self.gravity,
            Rfc5424::SYSLOG_VERSION,
            Rfc5424::format_timestamp(&self.timestamp),
            self.hostname,
            self.app_name,
            extract_first_span(&self.proc_id),
            "-",
            self.content,
        );
        msg
    }
}

impl Rfc5424Msg {
    /// Instantiate a rfc 5424 compliant message from a log event.
    ///
    /// # Arguments:
    ///
    /// * `event` - event received from the log queue.
    fn from_event(event: &Event) -> Result<Self, FormatterError> {
        if event.kind & 1 != 1 {
            // read this as !event.kind.is_event()
            return Err(FormatterError::NotAnEvent);
        }
        if let Some(msg) = vsmtp_log_dispatcher::get_message(event) {
            let mut rfc_formatted_msg = if msg.is_empty() { "-".to_string() } else { msg }
                .replace('{', "[")
                .replace('}', "]");
            if !rfc_formatted_msg.contains('[') {
                rfc_formatted_msg = format!("[{}]", rfc_formatted_msg);
            }

            Ok(Rfc5424Msg {
                facility: DEFAULT_SYSLOG_FACILITY,
                gravity: level_to_syslog_level(&event.level),
                timestamp: event.timestamp.into(),
                hostname: event.hostname.clone().unwrap_or("Unknown".to_string()), // TODO: IP address should be used if hostname is unknown
                app_name: "vSMTP".to_string(),
                proc_id: event.target.to_string(),
                content: rfc_formatted_msg,
            })
        } else {
            Err(FormatterError::NoMessage)
        }
    }
}

/// Formatter for rfc 5424.
pub struct Rfc5424;

impl Formatter for Rfc5424 {
    fn format(&self, event: &Event) -> Result<String, FormatterError> {
        match Rfc5424Msg::from_event(event) {
            Ok(msg) => Ok(msg.to_string()),
            Err(err) => Err(err),
        }
    }
}

impl Rfc5424 {
    /// Used syslog version.
    const SYSLOG_VERSION: u8 = 1;

    /// Format a timestamp as rfc 5424 compliant
    ///
    /// # Arguments:
    /// * `timestamp` timestamp to convert.
    fn format_timestamp(timestamp: &DateTime<Utc>) -> String {
        format!(
            "{}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            timestamp.year(),
            timestamp.month(),
            timestamp.day(),
            timestamp.hour(),
            timestamp.minute(),
            timestamp.second(),
        )
    }
}

/// Content of a rfc 3164 compliant message.
struct Rfc3164Msg {
    /// Syslog facility
    pub facility: u8,
    /// Gravity of the message (=level)
    pub gravity: u8,
    /// Timestamp of the message.
    pub timestamp: DateTime<Utc>,
    /// Hostname of the log emitter.
    pub hostname: String,
    /// Tag of the log message
    pub tag: String,
    /// Content of the log message
    pub content: String,
}

impl ToString for Rfc3164Msg {
    /// TODO:
    /// - remove quotes around message
    fn to_string(&self) -> String {
        let mut msg = format!(
            "<{}>{} {} {} {}",
            self.facility | self.gravity,
            Rfc3164::format_timestamp(&self.timestamp),
            self.hostname,
            self.tag,
            self.content
        );
        msg.truncate(1024);
        msg
    }
}

impl Rfc3164Msg {
    /// Instantiate a rfc 3164 compliant message from a log event.
    ///
    /// # Arguments:
    ///
    /// * `event` - event received from the log queue.
    fn from_event(event: &Event) -> Result<Self, FormatterError> {
        if event.kind & 1 != 1 {
            // read this as !event.kind.is_event()
            return Err(FormatterError::NotAnEvent);
        }
        if let Some(msg) = vsmtp_log_dispatcher::get_message(event) {
            Ok(Rfc3164Msg {
                facility: DEFAULT_SYSLOG_FACILITY,
                gravity: level_to_syslog_level(&event.level),
                timestamp: event.timestamp.into(),
                hostname: event.hostname.clone().unwrap_or("Unknown".to_string()), // TODO: IP address should be used if hostname is unknown
                tag: event.target.to_string(),
                content: msg,
            })
        } else {
            Err(FormatterError::NoMessage)
        }
    }
}

pub struct Rfc3164;

impl Rfc3164 {
    /// Format a timestamp as rfc 3164 compliant
    ///
    /// # Arguments:
    /// * `timestamp` timestamp to convert.
    fn format_timestamp(timestamp: &DateTime<Utc>) -> String {
        format!(
            "{} {:02} {:02}:{:02}:{:02}",
            Rfc3164::match_month(timestamp.month()),
            timestamp.day(),
            timestamp.hour(),
            timestamp.minute(),
            timestamp.second(),
        )
    }

    /// Convert a month digit to a three letters abbreviation.
    ///
    /// # Arguments:
    /// * `month` digit
    fn match_month(month: u32) -> &'static str {
        match month {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            12 => "Dec",
            _ => "Unknown",
        }
    }
}

impl Formatter for Rfc3164 {
    fn format(&self, event: &Event) -> Result<String, FormatterError> {
        match Rfc3164Msg::from_event(event) {
            Ok(msg) => Ok(msg.to_string()),
            Err(err) => Err(err),
        }
    }
}
