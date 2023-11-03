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

#[allow(unused_imports)]
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

pub use logging::*;

const DEFAULT_USER_LOG_TOPIC: &str = "user";

/// Logging mechanisms for rhai scripts.
#[rhai::plugin::export_module]
mod logging {

    /// Log information to a rabbitmq queue which can be retrieve via the log-dispatcher service.
    ///
    /// # Args
    ///
    /// * `target_topic` (default: "user") - the routing key used to route the log message ("user" by default).
    /// * `level` - the level of the message, can be "trace", "debug", "info", "warn" or "error".
    /// * `message` - the message to log.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```js
    /// log("my_queue", "error", "Hello world!");
    /// log("my_queue", "info", `[${date()}/${time()}] client=${ctx.client_ip}`);
    ///
    /// const level = "trace";
    /// const message = "connection established";
    /// log("my_queue", level, message);
    ///
    /// const level = "warn";
    /// log(level, "I love rhai!"); // this is send to "user" topic in logging queue
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, name = "log")]
    #[allow(clippy::cognitive_complexity)]
    pub fn log(target_topic: &str, level: &str, message: &str) {
        // Rename field for tracing.
        match <tracing::Level as std::str::FromStr>::from_str(level) {
            Ok(level) => match level {
                tracing::Level::TRACE => tracing::trace!(message, topic = target_topic),
                tracing::Level::DEBUG => tracing::debug!(message, topic = target_topic),
                tracing::Level::INFO => tracing::info!(message, topic = target_topic),
                tracing::Level::WARN => tracing::warn!(message, topic = target_topic),
                tracing::Level::ERROR => tracing::error!(message, topic = target_topic),
            },
            Err(e) => {
                tracing::warn!(
                    "level `{}` is invalid: `{}`. Message was: '{}'",
                    level,
                    e,
                    message,
                );
            }
        }
    }

    /// Log information to a rabbitmq queue which can be retrieve via the log-dispatcher service.
    /// The message is consider with a level error.
    ///
    /// # Args
    ///
    /// * `target_topic` (default: "user") - the routing key used to route the log message.
    /// * `message` - the message to log.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```js
    /// err("An error occurred");
    /// err("my_queue", "An error occurred");
    /// ```
    /// # rhai-autodocs:index:2
    #[rhai_fn(global, name = "err")]
    pub fn err(target_topic: &str, message: &str) {
        tracing::error!(message, topic = target_topic);
    }

    /// Log information to a rabbitmq queue which can be retrieve via the log-dispatcher service.
    /// The message is consider with a level warning.
    ///
    /// # Args
    ///
    /// * `target_topic` (default: "user") - the routing key used to route the log message ("user" by default).
    /// * `message` - the message to log.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```js
    /// warn("warning!");
    /// warn("my_queue", "warning!");
    /// ```
    /// # rhai-autodocs:index:3
    #[rhai_fn(global, name = "warn")]
    pub fn warn(target_topic: &str, message: &str) {
        tracing::warn!(message, topic = target_topic);
    }

    /// Log information to a rabbitmq queue which can be retrieve via the log-dispatcher service.
    /// The message is consider with a level info.
    ///
    /// # Args
    ///
    /// * `target_topic` (default: "user") - the routing key used to route the log message ("user" by default).
    /// * `message` - the message to log.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```js
    /// info("New info just dropped");
    /// info("my_queue", "New info just dropped");
    /// ```
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, name = "info")]
    pub fn info(target_topic: &str, message: &str) {
        tracing::info!(message, topic = target_topic);
    }

    /// Log information to a rabbitmq queue which can be retrieve via the log-dispatcher service.
    /// The message is consider with a level debug.
    ///
    /// # Args
    ///
    /// * `target_topic` (default: "user") - the routing key used to route the log message ("user" by default).
    /// * `message` - the message to log.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```js
    /// debug("Debugging stuff");
    /// debug("my_queue", "Debugging stuff");
    /// ```
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, name = "debug")]
    pub fn debug(target_topic: &str, message: &str) {
        tracing::debug!(message, topic = target_topic);
    }

    /// Log information to a rabbitmq queue which can be retrieve via the log-dispatcher service.
    /// The message is consider with a level trace.
    ///
    /// # Args
    ///
    /// * `target_topic` (default: "user") - the routing key used to route the log message ("user" by default).
    /// * `message` - the message to log.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```js
    /// trace(`client_ip=${ctx.client_ip}`);
    /// trace("my_queue", `client_ip=${ctx.client_ip}`);
    /// ```
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, name = "trace")]
    pub fn trace(target_topic: &str, message: &str) {
        tracing::trace!(message, topic = target_topic);
    }

    #[doc(hidden)]
    #[rhai_fn(global, name = "log")]
    #[allow(clippy::cognitive_complexity)]
    pub fn log_default_topic(level: &str, message: &str) {
        match <tracing::Level as std::str::FromStr>::from_str(level) {
            Ok(level) => match level {
                tracing::Level::TRACE => {
                    tracing::trace!(message, topic = DEFAULT_USER_LOG_TOPIC);
                }
                tracing::Level::DEBUG => {
                    tracing::debug!(message, topic = DEFAULT_USER_LOG_TOPIC);
                }
                tracing::Level::INFO => {
                    tracing::info!(message, topic = DEFAULT_USER_LOG_TOPIC);
                }
                tracing::Level::WARN => {
                    tracing::warn!(message, topic = DEFAULT_USER_LOG_TOPIC);
                }
                tracing::Level::ERROR => {
                    tracing::error!(message, topic = DEFAULT_USER_LOG_TOPIC);
                }
            },
            Err(e) => {
                tracing::warn!(
                    "level `{}` is invalid: `{}`. Message was: '{}'",
                    level,
                    e,
                    message,
                );
            }
        }
    }

    #[doc(hidden)]
    #[rhai_fn(global, name = "err")]
    pub fn err_default_topic(message: &str) {
        tracing::error!(message, topic = DEFAULT_USER_LOG_TOPIC);
    }

    #[doc(hidden)]
    #[rhai_fn(global, name = "warn")]
    pub fn warn_default_topic(message: &str) {
        tracing::warn!(message, topic = DEFAULT_USER_LOG_TOPIC);
    }

    #[doc(hidden)]
    #[rhai_fn(global, name = "info")]
    pub fn info_default_topic(message: &str) {
        tracing::info!(message, topic = DEFAULT_USER_LOG_TOPIC);
    }

    #[doc(hidden)]
    #[rhai_fn(global, name = "debug")]
    pub fn debug_default_topic(message: &str) {
        tracing::debug!(message, topic = DEFAULT_USER_LOG_TOPIC);
    }

    #[doc(hidden)]
    #[rhai_fn(global, name = "trace")]
    pub fn trace_default_topic(message: &str) {
        tracing::trace!(message, topic = DEFAULT_USER_LOG_TOPIC);
    }
}
