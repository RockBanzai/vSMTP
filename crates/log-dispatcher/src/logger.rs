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

use std::{
    io::Write,
    net::{TcpStream, UdpSocket},
    os::unix::net::{UnixDatagram, UnixStream},
};

use crate::{
    config,
    formatter::{self, Formatter},
};
use colored::Colorize;
use tracing_amqp::Event;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

pub trait Logger {
    fn log(&mut self, event: &Event);
}

/// Console logger
#[derive(Default)]
pub struct Console {
    /// Optional log formatter
    formatter: Option<Box<dyn Formatter>>,
}

impl Console {
    /// Format a level for the console
    ///
    /// # Arguments:
    /// * `level` level to format
    fn format_level(level: tracing::Level) -> String {
        let text_level = vsmtp_log_dispatcher::format_level(level);
        match level {
            tracing::Level::ERROR => text_level.red(),
            tracing::Level::WARN => text_level.yellow(),
            tracing::Level::INFO => text_level.green(),
            tracing::Level::DEBUG => text_level.blue(),
            tracing::Level::TRACE => text_level.purple(),
        }
        .to_string()
    }

    /// Set a formatter
    ///
    /// # Arguments:
    /// * `formatter` pointer to a formatter.
    pub fn set_formatter(&mut self, formatter: Box<dyn Formatter>) {
        self.formatter = Some(formatter);
    }
}

impl Logger for Console {
    fn log(&mut self, event: &Event) {
        if let Some(formatter) = &self.formatter {
            match formatter.format(event) {
                Ok(msg) => println!("{msg}"),
                Err(err) => tracing::warn!("Cannot not display received log: {}", err.to_string()),
            }
        } else {
            match vsmtp_log_dispatcher::get_message(event) {
                Some(msg) => println!(
                    "{} {} {} {}: {}",
                    vsmtp_log_dispatcher::format_timestamp(&event.timestamp.into()),
                    Self::format_level(event.level),
                    event.service,
                    if cfg!(debug_assertions) {
                        event.target.to_string().italic()
                    } else {
                        formatter::extract_first_span(&event.target.to_string()).italic()
                    },
                    msg
                ),
                None => tracing::warn!("Cannot not display received log, this is a bug"),
            }
        }
    }
}

/// File logger
pub struct File {
    /// Manager for the log files
    file_appender: RollingFileAppender,
}

impl File {
    /// Instantiate a new file logger
    ///
    /// # Arguments:
    /// * `rotation` control the max age of a log file
    /// * `folder` folder on which logs are stored
    /// * `file_prefix` prefix added in front of log files names
    pub fn new(rotation: config::FileRotation, folder: String, file_prefix: String) -> Self {
        let rotation = match rotation {
            config::FileRotation::Never => Rotation::NEVER,
            config::FileRotation::Daily => Rotation::DAILY,
            config::FileRotation::Hourly => Rotation::HOURLY,
            config::FileRotation::Minutely => Rotation::MINUTELY,
        };
        Self {
            file_appender: RollingFileAppender::new(rotation, folder, file_prefix),
        }
    }

    fn format(event: &Event) -> Option<String> {
        vsmtp_log_dispatcher::get_message(event).map(|msg| {
            format!(
                "{} {} {}",
                vsmtp_log_dispatcher::format_timestamp(&event.timestamp.into()),
                vsmtp_log_dispatcher::format_level(event.level),
                msg,
            )
        })
    }
}

impl Logger for File {
    fn log(&mut self, event: &Event) {
        if let Some(mut msg) = Self::format(event) {
            msg.push('\n');
            if let Err(err) = self.file_appender.write(msg.as_bytes()) {
                tracing::warn!("Cannot write log to log file: {}", err);
            }
        }
    }
}

/// Syslog logger
pub struct Syslog {
    /// Optional log formatter
    formatter: Box<dyn Formatter>,
    /// Protocol used to communicate to syslog
    protocol: config::SyslogProtocol,
    /// Address of the syslog service
    address: String,
    /// Tcp stream created if protocol is tcp
    tcp_stream: Option<TcpStream>,
    /// Udp Socket created if protocol is udp
    udp_socket: Option<UdpSocket>,
    /// Unix socket created if protocol is unix socket
    unix_socket: Option<UnixDatagram>,
    /// Unix stream created if protocol is unix socket stream
    unix_stream: Option<UnixStream>,
}

impl Syslog {
    /// Instantiate a new Syslog logger
    ///
    /// # Arguments:
    /// * `protocol` protocol used to communicate with syslog service
    /// * `address` address of the syslog service
    /// * `formatter` optional logs formatter
    pub fn new(
        protocol: config::SyslogProtocol,
        address: String,
        formatter: Box<dyn Formatter>,
    ) -> Self {
        let mut instance = Self {
            protocol,
            address,
            formatter,
            tcp_stream: None,
            udp_socket: None,
            unix_socket: None,
            unix_stream: None,
        };
        match instance.protocol {
            config::SyslogProtocol::Tcp => {
                instance.tcp_stream = match TcpStream::connect(instance.address.clone()) {
                    Ok(stream) => Some(stream),
                    Err(err) => {
                        tracing::warn!(
                            "Failed to connect to a syslog service with the address {}: {}",
                            instance.address,
                            err
                        );
                        None
                    }
                }
            }
            config::SyslogProtocol::Udp => {
                instance.udp_socket = match UdpSocket::bind("127.0.0.1:0") {
                    Ok(socket) => match socket.connect(instance.address.clone()) {
                        Ok(()) => Some(socket),
                        Err(err) => {
                            tracing::warn!(
                                "Failed to connect to a syslog service with the address {}: {}",
                                instance.address,
                                err
                            );
                            None
                        }
                    },
                    Err(err) => {
                        tracing::warn!("Failed to bind socket to localhost: {}", err);
                        None
                    }
                }
            }
            config::SyslogProtocol::UnixSocket => {
                instance.unix_socket = match UnixDatagram::unbound() {
                    Ok(socket) => Some(socket),
                    Err(err) => {
                        tracing::warn!("Cannot unbound the journald socket: {}", err);
                        None
                    }
                }
            }
            config::SyslogProtocol::UnixSocketStream => {
                instance.unix_stream = match UnixStream::connect(instance.address.clone()) {
                    Ok(stream) => Some(stream),
                    Err(err) => {
                        tracing::warn!(
                            "Failed to connect to a syslog service with the address {}: {}",
                            instance.address,
                            err
                        );
                        None
                    }
                }
            }
        }
        instance
    }
}

impl Logger for Syslog {
    fn log(&mut self, event: &Event) {
        if let Ok(msg) = self.formatter.format(event) {
            match self.protocol {
                config::SyslogProtocol::Tcp => {
                    if let Some(stream) = &mut self.tcp_stream {
                        if let Err(err) = stream.write_all(msg.as_bytes()) {
                            tracing::warn!("Cannot send message to syslog: {err}");
                        }
                    }
                }
                config::SyslogProtocol::Udp => {
                    if let Some(socket) = &mut self.udp_socket {
                        if let Err(err) = socket.send(msg.as_bytes()) {
                            tracing::warn!("Cannot send message to syslog: {err}");
                        }
                    }
                }
                config::SyslogProtocol::UnixSocket => {
                    if let Some(socket) = &mut self.unix_socket {
                        if let Err(err) = socket.send_to(msg.as_bytes(), self.address.clone()) {
                            tracing::warn!("Cannot send message to syslog: {err}");
                        }
                    }
                }
                config::SyslogProtocol::UnixSocketStream => {
                    if let Some(stream) = &mut self.tcp_stream {
                        if let Err(err) = stream.write_all(msg.as_bytes()) {
                            tracing::warn!("Cannot send message to syslog: {err}");
                        }
                    }
                }
            }
        }
    }
}

/// Journald logger
pub struct Journald {
    /// socket to journald
    socket: Option<UnixDatagram>,
}

impl Logger for Journald {
    fn log(&mut self, event: &Event) {
        if let Some(socket) = &mut self.socket {
            if let Some(msg) = vsmtp_log_dispatcher::get_message(event) {
                let msg = Self::format_event(event, &msg);
                if let Err(err) = socket.send_to(msg.as_bytes(), Self::SYSTEMD_SOCKET) {
                    tracing::warn!("Cannot send message to journald: {err}");
                }
            }
        }
    }
}

impl Journald {
    const SYSTEMD_SOCKET: &'static str = "/run/systemd/journal/socket";

    /// Format an event to be syslog compliant
    ///
    /// # Arguments:
    /// * `event` the event to format
    /// * `message` the message to send
    fn format_event(event: &Event, msg: &String) -> String {
        // TODO: handle '\n' logging (see https://systemd.io/JOURNAL_NATIVE_PROTOCOL/)
        let mut msg = format!(
            "MESSAGE={}\nPRIORITY={}\n",
            msg,
            formatter::level_to_syslog_level(&event.level),
        );
        if let Some(file) = event.file {
            msg.push_str(format!("CODE_FILE={file}\n").as_str());
        }
        if let Some(line) = event.line {
            msg.push_str(format!("CODE_LINE={line}\n").as_str());
        }
        msg
    }

    /// Instantiate a new journald logger
    pub fn new() -> Self {
        let socket = match UnixDatagram::unbound() {
            Ok(socket) => Some(socket),
            Err(err) => {
                tracing::warn!("Cannot unbound the journald socket: {}", err);
                None
            }
        };
        Self { socket }
    }
}
