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

/// Logging mechanisms.
#[rhai::plugin::export_module]
mod logging {

    /// Log information to stdout in `nodaemon` mode or to a file.
    ///
    /// # Args
    ///
    /// * `level` - the level of the message, can be "trace", "debug", "info", "warn" or "error".
    /// * `message` - the message to log.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///   connect: [
    ///     action "log on connection (str/str)" || {
    ///       log("info", `[${date()}/${time()}] client=${ctx::client_ip()}`);
    ///     },
    ///     action "log on connection (str/obj)" || {
    ///       log("error", identifier("Hello world!"));
    ///     },
    ///     action "log on connection (obj/obj)" || {
    ///       const level = "trace";
    ///       const message = "connection established";
    ///
    ///       log(identifier(level), identifier(message));
    ///     },
    ///     action "log on connection (obj/str)" || {
    ///       const level = "warn";
    ///
    ///       log(identifier(level), "I love rhai!");
    ///     },
    ///   ],
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, name = "log")]
    #[allow(clippy::cognitive_complexity)]
    pub fn log(level: &str, message: &str) {
        match <tracing::Level as std::str::FromStr>::from_str(level) {
            Ok(level) => match level {
                tracing::Level::TRACE => tracing::trace!(message),
                tracing::Level::DEBUG => tracing::debug!(message),
                tracing::Level::INFO => tracing::info!(message),
                tracing::Level::WARN => tracing::warn!(message),
                tracing::Level::ERROR => tracing::error!(message),
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
}
