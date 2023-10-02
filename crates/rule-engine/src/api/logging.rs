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

/// Logging mechanisms.
#[rhai::plugin::export_module]
mod logging {

    /// Log information to a rabbitmq queue which can be retrieve via the log-dispatcher service.
    ///
    /// # Args
    ///
    /// * `target_topic` - the queue on which the log is sent.
    /// * `level` - the level of the message, can be "trace", "debug", "info", "warn" or "error".
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```js
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///   connect: [
    ///     action "log on connection (str/str)" || {
    ///       log("my_queue", "info", `[${date()}/${time()}] client=${ctx::client_ip()}`);
    ///     },
    ///     action "log on connection (str/obj)" || {
    ///       log("my_queue", "error", identifier("Hello world!"));
    ///     },
    ///     action "log on connection (obj/obj)" || {
    ///       const level = "trace";
    ///       const message = "connection established";
    ///
    ///       log("my_queue", identifier(level), identifier(message));
    ///     },
    ///     action "log on connection (obj/str)" || {
    ///       const level = "warn";
    ///
    ///       log("my_queue", identifier(level), "I love rhai!");
    ///     },
    ///   ],
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, name = "log")]
    #[allow(clippy::cognitive_complexity)]
    pub fn log(target_topic: &str, level: &str, message: &str) {
        match <tracing::Level as std::str::FromStr>::from_str(level) {
            // 'target_topic' field is not called 'topic' to avoid overriding the topic of tracing API.
            Ok(level) => match level {
                tracing::Level::TRACE => tracing::trace!(message, target_topic),
                tracing::Level::DEBUG => tracing::debug!(message, target_topic),
                tracing::Level::INFO => tracing::info!(message, target_topic),
                tracing::Level::WARN => tracing::warn!(message, target_topic),
                tracing::Level::ERROR => tracing::error!(message, target_topic),
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

    /// Log information to a rabbitmq log which can be retrieve via the log-dispatcher service.
    /// The message is consider as an error.
    ///
    /// # Args
    ///
    /// * `target_topic` - the queue on which the log is sent.
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "err")]
    #[allow(clippy::cognitive_complexity)]
    pub fn err(target_topic: &str, message: &str) {
        tracing::error!(message, target_topic)
    }

    /// Log information to a rabbitmq log which can be retrieve via the log-dispatcher service.
    /// The message is consider as a warning.
    ///
    /// # Args
    ///
    /// * `target_topic` - the queue on which the log is sent.
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "warn")]
    #[allow(clippy::cognitive_complexity)]
    pub fn warn(target_topic: &str, message: &str) {
        tracing::warn!(message, target_topic)
    }

    /// Log information to a rabbitmq log which can be retrieve via the log-dispatcher service.
    /// The message is consider as an info.
    ///
    /// # Args
    ///
    /// * `target_topic` - the queue on which the log is sent.
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "info")]
    #[allow(clippy::cognitive_complexity)]
    pub fn info(target_topic: &str, message: &str) {
        tracing::info!(message, target_topic)
    }

    /// Log information to a rabbitmq log which can be retrieve via the log-dispatcher service.
    /// The message is consider as a debug.
    ///
    /// # Args
    ///
    /// * `target_topic` - the queue on which the log is sent.
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "debug")]
    #[allow(clippy::cognitive_complexity)]
    pub fn debug(target_topic: &str, message: &str) {
        tracing::debug!(message, target_topic)
    }

    /// Log information to a rabbitmq log which can be retrieve via the log-dispatcher service.
    /// The message is consider as a trace.
    ///
    /// # Args
    ///
    /// * `target_topic` - the queue on which the log is sent.
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "trace")]
    #[allow(clippy::cognitive_complexity)]
    pub fn trace(target_topic: &str, message: &str) {
        tracing::trace!(message, target_topic)
    }

    /// Log information to the default rabbitmq queue (system) which can be retrieve via the log-dispatcher service.
    ///
    /// # Args
    ///
    /// * `level` - the level of the message, can be "trace", "debug", "info", "warn" or "error".
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "log")]
    #[allow(clippy::cognitive_complexity)]
    pub fn log_default_target(level: &str, message: &str) {
        match <tracing::Level as std::str::FromStr>::from_str(level) {
            // 'target_topic' field is not called 'topic' to avoid overriding the topic of tracing API.
            Ok(level) => match level {
                tracing::Level::TRACE => {
                    tracing::trace!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
                }
                tracing::Level::DEBUG => {
                    tracing::debug!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
                }
                tracing::Level::INFO => {
                    tracing::info!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
                }
                tracing::Level::WARN => {
                    tracing::warn!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
                }
                tracing::Level::ERROR => {
                    tracing::error!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
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

    /// Log information to the default rabbitmq queue (system) which can be retrieve via the log-dispatcher service.
    /// The message is consider as an error.
    ///
    /// # Args
    ///
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "err_default_target")]
    #[allow(clippy::cognitive_complexity)]
    pub fn err_default_target(message: &str) {
        tracing::error!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
    }

    /// Log information to the default rabbitmq queue (system) which can be retrieve via the log-dispatcher service.
    /// The message is consider a warning.
    ///
    /// # Args
    ///
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "warn_default_target")]
    #[allow(clippy::cognitive_complexity)]
    pub fn warn_default_target(message: &str) {
        tracing::warn!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
    }

    /// Log information to the default rabbitmq queue (system) which can be retrieve via the log-dispatcher service.
    /// The message is consider as an info.
    ///
    /// # Args
    ///
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "info_default_target")]
    #[allow(clippy::cognitive_complexity)]
    pub fn info_default_target(message: &str) {
        tracing::info!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
    }

    /// Log information to the default rabbitmq queue (system) which can be retrieve via the log-dispatcher service.
    /// The message is consider as a debug.
    ///
    /// # Args
    ///
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "debug_default_target")]
    #[allow(clippy::cognitive_complexity)]
    pub fn debug_default_target(message: &str) {
        tracing::debug!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
    }

    /// Log information to the default rabbitmq queue (system) which can be retrieve via the log-dispatcher service.
    /// The message is consider as a trace.
    ///
    /// # Args
    ///
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    #[rhai_fn(global, name = "trace_default_target")]
    #[allow(clippy::cognitive_complexity)]
    pub fn trace_default_target(message: &str) {
        tracing::trace!(message, target_topic = DEFAULT_USER_LOG_TOPIC)
    }
}
