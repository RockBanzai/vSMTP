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
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_rule_engine::rhai;

/// Functions used to interact with the rule engine.
/// Use `states` in `rules` to deny, accept, or quarantine emails.
#[rhai::plugin::export_module]
pub mod status {
    use crate::rules::status::WorkingStatus;

    /// Tell the rule engine that a rule succeeded. Following rules
    /// in the current stage will be executed.
    ///
    /// # Effective smtp stage
    ///
    /// ```post_queue```
    ///
    /// # Example
    ///
    /// ```js title="/etc/vsmtp/working/script.rhai"
    /// fn on_post_queue(ctx) {
    ///     status::next()
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[must_use]
    pub const fn next() -> WorkingStatus {
        WorkingStatus::Next
    }

    /// Tell the rule engine that the mail we are working with does
    /// not need further processing.
    /// This means that all rules following the `success` call
    /// will be ignored.
    ///
    /// # Effective smtp stage
    ///
    /// ```post_queue```
    ///
    /// # Example
    ///
    /// ```js title="/etc/vsmtp/working/script.rhai"
    /// fn on_post_queue(ctx) {
    ///     status::success()
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[must_use]
    pub const fn success() -> WorkingStatus {
        WorkingStatus::Success
    }

    // FIXME: in which queue the email should be stored on failure ?
    /// Stops processing because of a failure.
    /// The email will be stored in the `dead` queue, and all further rules
    /// are skipped.
    ///
    /// # Effective smtp stage
    ///
    /// ```post_queue```
    ///
    /// # Example
    ///
    /// ```js title="/etc/vsmtp/working/script.rhai"
    /// fn on_post_queue(ctx) {
    ///     status::fail()
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:3
    #[must_use]
    pub const fn fail() -> WorkingStatus {
        WorkingStatus::Fail
    }

    /// Skip all rules and place the email in a quarantine queue.
    /// The email will never be sent to the recipients and
    /// will stop being processed.
    ///
    /// # Args
    ///
    /// * `queue` - the relative path to the queue where the email will be quarantined as a string.
    ///             This path will be concatenated to the `config.app.dirpath` field in
    ///             your root configuration.
    ///
    /// # Effective smtp stage
    ///
    /// ```post_queue```
    ///
    /// # Example
    ///
    /// ```js title="/etc/vsmtp/working/script.rhai"
    /// fn on_post_queue(ctx) {
    ///     status::quarantine("virus")
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:4
    #[must_use]
    pub fn quarantine(queue: &str) -> WorkingStatus {
        WorkingStatus::Quarantine(queue.to_string())
    }

    /// Check if two statuses are equal.
    ///
    /// # Effective smtp stage
    ///
    /// ```post_queue```
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, name = "==", pure)]
    pub fn eq_status_operator(status_1: &mut WorkingStatus, status_2: WorkingStatus) -> bool {
        *status_1 == status_2
    }

    /// Check if two statuses are not equal.
    ///
    /// # Effective smtp stage
    ///
    /// ```post_queue```
    ///
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, name = "!=", pure)]
    pub fn neq_status_operator(status_1: &mut WorkingStatus, status_2: WorkingStatus) -> bool {
        !(*status_1 == status_2)
    }

    /// Convert a status to a string.
    /// Enables string interpolation.
    ///
    /// # Effective smtp stage
    ///
    /// ```post_queue```
    ///
    /// # rhai-autodocs:index:7
    #[rhai_fn(global, pure)]
    pub fn to_string(status: &mut WorkingStatus) -> String {
        status.as_ref().to_string()
    }

    /// Convert a status to a debug string
    /// Enables string interpolation.
    ///
    /// # Effective smtp stage
    ///
    /// ```post_queue```
    ///
    /// # rhai-autodocs:index:8
    #[rhai_fn(global, pure)]
    pub fn to_debug(status: &mut WorkingStatus) -> String {
        format!("{status:?}")
    }
}
