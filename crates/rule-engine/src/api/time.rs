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

use rhai::plugin::{
    Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult, TypeId,
};

const DATE_FORMAT: &[time::format_description::FormatItem<'_>] =
    time::macros::format_description!("[year]-[month]-[day]");
const TIME_FORMAT: &[time::format_description::FormatItem<'_>] =
    time::macros::format_description!("[hour]:[minute]:[second]");

pub use time_mod::*;

/// Utilities to get the current time and date.
#[rhai::plugin::export_module]
mod time_mod {
    /// Get the current time.
    ///
    /// # Return
    ///
    /// * `string` - the current time.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```text
    /// #{
    ///     preq: [
    ///        action "append info header" || {
    ///             msg::append_header("X-VSMTP", `email received by ${utils::hostname()} the ${time::date()} at ${time::now()}.`);
    ///        }
    ///     ]
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[must_use]
    pub fn now() -> String {
        let now = time::OffsetDateTime::now_utc();

        now.format(&TIME_FORMAT)
            .unwrap_or_else(|_| String::default())
    }

    /// Get the current date.
    ///
    /// # Return
    ///
    /// * `string` - the current date.
    ///
    /// # Effective smtp stage
    ///
    /// All of them.
    ///
    /// # Examples
    ///
    /// ```text
    /// #{
    ///     preq: [
    ///        action "append info header" || {
    ///             msg::append_header("X-VSMTP", `email received by ${utils::hostname()} the ${time::date()}.`);
    ///        }
    ///     ]
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[must_use]
    pub fn date() -> String {
        let now = time::OffsetDateTime::now_utc();

        now.format(&DATE_FORMAT)
            .unwrap_or_else(|_| String::default())
    }
}
