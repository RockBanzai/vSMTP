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

/// Utility functions to interact with the system.
///
/// This modules is accessible in filtering AND configuration scripts.
#[rhai::plugin::export_module]
pub mod api {
    /// Fetch an environment variable from the current process.
    ///
    /// # Args
    ///
    /// * `variable` - the variable to fetch.
    ///
    /// # Returns
    ///
    /// * `string` - the value of the fetched variable.
    /// * `()`     - when the variable is not set, when the variable contains the character `=` or `\0`
    ///             or that the variable is not unicode valid.
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///   connect: [
    ///     rule "get env variable" || {
    ///
    ///       // get the HOME environment variable.
    ///       let home = env::variable("HOME");
    ///
    /// #       if home == () {
    /// #           return state::deny(`500 home,${home}`);
    /// #       }
    ///
    ///       // "VSMTP=ENV" is malformed, this will return the unit type '()'.
    ///       let invalid = env::variable("VSMTP=ENV");
    ///
    /// #       if invalid != () {
    /// #           return state::deny(`500 invalid,${invalid}`);
    /// #       }
    ///
    /// #       state::accept(`250 test ok`)
    ///       // ...
    ///     }
    ///   ],
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::Status, Reply, ReplyCode::Code};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::Connect].2, Status::Accept(
    /// #  "250 test ok".parse().unwrap(),
    /// # ));
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, name = "variable")]
    pub fn variable_str(variable: &str) -> rhai::Dynamic {
        std::env::var(variable).map_or(rhai::Dynamic::UNIT, std::convert::Into::into)
    }
}
