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

use crate::smtp::rules::status::ReceiverStatus;
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_protocol::Reply;
use vsmtp_rule_engine::rhai;

type Result<T> = std::result::Result<T, Box<rhai::EvalAltResult>>;

/// Create a reply object and map to the appropriate error.
fn reply_from_string(code: &str) -> Result<Reply> {
    <Reply as std::str::FromStr>::from_str(code).map_err::<Box<EvalAltResult>, _>(|_| {
        format!("parameter must be a code, not {code:?}").into()
    })
}

/// Functions used to interact with the rule engine.
/// Use `states` in `rules` to deny, accept, or quarantine emails.
#[rhai::plugin::export_module]
pub mod status {

    /// Tell the rule engine to accept the incoming transaction for the current stage.
    /// This means that all rules following the one `accept` is called in the current stage
    /// will be ignored.
    ///
    /// # Args
    ///
    /// * code - A customized code as a string or code object. (default: "250 Ok")
    ///
    /// # Errors
    ///
    /// * The object passed as parameter was not a code object.
    /// * The string passed as parameter failed to be parsed into a valid code.
    ///
    /// # Effective smtp stage
    ///
    /// all of them.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #{
    ///     connect: [
    ///         // "ignored checks" will be ignored because the previous rule returned accept.
    ///         rule "accept" || state::accept(),
    ///         action "ignore checks" || print("this will be ignored because the previous rule used state::accept()."),
    ///     ],
    ///
    ///     mail: [
    ///         // rule evaluation is resumed in the next stage.
    ///         rule "resume rules" || print("evaluation resumed!");
    ///     ]
    /// }
    ///
    /// #{
    ///     mail: [
    ///         rule "send a custom code with a code object" || {
    ///             accept(code(220, "Ok"))
    ///         }
    ///     ],
    /// }
    ///
    /// #{
    ///     mail: [
    ///         rule "send a custom code with a string" || {
    ///             accept("220 Ok")
    ///         }
    ///     ],
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[must_use]
    pub const fn accept() -> ReceiverStatus {
        ReceiverStatus::Accept(None)
    }

    #[doc(hidden)]
    #[rhai_fn(name = "accept", return_raw)]
    pub fn accept_with_string(code: &str) -> Result<ReceiverStatus> {
        reply_from_string(code).map(|reply| ReceiverStatus::Accept(Some(reply)))
    }

    /// Tell the rule engine that a rule succeeded. Following rules
    /// in the current stage will be executed.
    ///
    /// # Effective smtp stage
    ///
    /// all of them.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #{
    ///     connect: [
    ///         // once "go to the next rule" is evaluated, the rule engine execute "another rule".
    ///         rule "go to the next rule" || state::next(),
    ///         action "another rule" || print("checking stuff ..."),
    ///     ],
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[must_use]
    pub const fn next() -> ReceiverStatus {
        ReceiverStatus::Next
    }

    /// Sends an error code to the client and closes the transaction.
    ///
    /// # Args
    ///
    /// * code - A customized code as a string or code object. (default: "554 permanent problems with the remote server")
    ///
    /// # Errors
    ///
    /// * The object passed as parameter was not a code object.
    /// * The string passed as parameter failed to be parsed into a valid code.
    ///
    /// # Effective smtp stage
    ///
    /// all of them.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #{
    ///     rcpt: [
    ///         rule "check for satan" || {
    ///            // The client is denied if a recipient's domain matches satan.org,
    ///            // this is a blacklist, sort-of.
    ///            if ctx::rcpt().domain == "satan.org" {
    ///                state::deny()
    ///            } else {
    ///                state::next()
    ///            }
    ///        },
    ///     ],
    /// }
    ///
    /// #{
    ///     mail: [
    ///         rule "send a custom code with a code object" || {
    ///             deny(code(421, "Service not available"))
    ///         }
    ///     ],
    /// }
    ///
    /// #{
    ///     mail: [
    ///         rule "send a custom code with a string" || {
    ///             deny("450 mailbox unavailable")
    ///         }
    ///     ],
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:3
    #[must_use]
    #[rhai_fn(global)]
    pub const fn deny() -> ReceiverStatus {
        ReceiverStatus::Deny(None)
    }

    #[doc(hidden)]
    #[rhai_fn(name = "deny", return_raw)]
    pub fn deny_with_string(code: &str) -> Result<ReceiverStatus> {
        reply_from_string(code).map(|reply| ReceiverStatus::Deny(Some(reply)))
    }

    /// Skip all rules until the email is received and place the email in a
    /// quarantine queue. The email will never be sent to the recipients and
    /// will stop being processed after the `PreQ` stage.
    ///
    /// # Args
    ///
    /// * `queue` - the relative path to the queue where the email will be quarantined as a string.
    ///             This path will be concatenated to the `config.app.dirpath` field in
    ///             your root configuration.
    ///
    /// # Effective smtp stage
    ///
    /// all of them.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #{
    ///     postq: [
    ///           rule "check email for virus" || {
    ///               // the email is placed in quarantined if a virus is detected by
    ///               // a service.
    ///               if has_header("X-Virus-Infected") {
    ///                 state::quarantine("virus_queue")
    ///               } else {
    ///                 state::next()
    ///               }
    ///           }
    ///     ],
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:4
    #[must_use]
    #[rhai_fn(name = "quarantine")]
    pub fn quarantine_str(queue: &str) -> ReceiverStatus {
        ReceiverStatus::Quarantine(queue.to_string(), None)
    }

    /// Check if two statuses are equal.
    ///
    /// # Effective smtp stage
    ///
    /// all of them.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #{
    ///     connect: [
    ///         action "check status equality" || {
    ///             deny() == deny(); // returns true.
    ///             faccept() == next(); // returns false.
    ///         }
    ///     ],
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, name = "==", pure)]
    pub fn eq_status_operator(status_1: &mut ReceiverStatus, status_2: ReceiverStatus) -> bool {
        *status_1 == status_2
    }

    /// Check if two statuses are not equal.
    ///
    /// # Effective smtp stage
    ///
    /// all of them.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #{
    ///     connect: [
    ///         action "check status not equal" || {
    ///             deny() != deny(); // returns false.
    ///             faccept() != next(); // returns true.
    ///         }
    ///     ],
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, name = "!=", pure)]
    pub fn neq_status_operator(status_1: &mut ReceiverStatus, status_2: ReceiverStatus) -> bool {
        !(*status_1 == status_2)
    }

    /// Convert a status to a string.
    /// Enables string interpolation.
    ///
    /// # Effective smtp stage
    ///
    /// all of them.
    ///
    /// # Example
    ///
    /// ```text,ignore
    /// #{
    ///     connect: [
    ///         rule "status to string" || {
    ///             let status = next();
    ///             // `.to_string` is called automatically here.
    ///             log("info", `converting my status to a string: ${status}`);
    ///             status
    ///         }
    ///     ],
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:7
    #[rhai_fn(global, pure)]
    pub fn to_string(status: &mut ReceiverStatus) -> String {
        status.to_string()
    }

    /// Convert a status to a debug string
    /// Enables string interpolation.
    ///
    /// # Effective smtp stage
    ///
    /// all of them.
    ///
    /// # Example
    ///
    /// ```text,ignore
    /// #{
    ///     connect: [
    ///         rule "status to string" || {
    ///             let status = next();
    ///             log("info", `converting my status to a string: ${status.to_debug()}`);
    ///             status
    ///         }
    ///     ],
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:8
    #[rhai_fn(global, pure)]
    pub fn to_debug(status: &mut ReceiverStatus) -> String {
        format!("{status:?}")
    }
}

/// Predefined codes for SMTP responses.
#[rhai::plugin::export_module]
pub mod code {
    pub type Code = Reply;

    /// A SMTP code with the code and message as parameter and an enhanced code.
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, name = "code", return_raw)]
    pub fn code_enhanced(code: rhai::INT, enhanced: &str, text: &str) -> Result<Code> {
        format!("{code} {enhanced} {text}")
            .parse::<Code>()
            .map_err(|error| error.to_string().into())
    }

    /// Return a relay access denied code.
    ///
    /// # Example
    ///
    /// ```
    /// # // Returning a access denied code in mail stage is stupid, but it works as an example.
    /// # // Could not make it work at the rcpt stage.
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "554 5.7.1 Relay access denied" to the client.
    ///         rule "anti relay" || { state::deny(code::c554_7_1()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "554 5.7.1 Relay access denied\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(name = "c554_7_1")]
    pub fn c554_7_1() -> Code {
        code_enhanced(554, "5.7.1", "Relay access denied").expect("valid code")
    }

    /// Return a DKIM Failure code. (RFC 6376)
    /// DKIM signature not found.
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "550 5.7.20 No passing DKIM signature found" to the client.
    ///         rule "deny with code" || { state::deny(code::c550_7_20()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "550 5.7.20 No passing DKIM signature found\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:3
    #[rhai_fn(name = "c550_7_20")]
    pub fn c550_7_20() -> Code {
        code_enhanced(550, "5.7.20", "No passing DKIM signature found").expect("valid code")
    }

    /// Return a DKIM Failure code. (RFC 6376)
    /// No acceptable DKIM signature found.
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "550 5.7.21 No acceptable DKIM signature found" to the client.
    ///         rule "deny with code" || { state::deny(code::c550_7_21()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #    "550 5.7.21 No acceptable DKIM signature found\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:4
    #[rhai_fn(name = "c550_7_21")]
    pub fn c550_7_21() -> Code {
        code_enhanced(550, "5.7.21", "No acceptable DKIM signature found").expect("valid code")
    }

    /// Return a DKIM Failure code. (RFC 6376)
    /// No valid author matched DKIM signature found.
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "550 5.7.22 No valid author-matched DKIM signature found" to the client.
    ///         rule "deny with code" || { state::deny(code::c550_7_22()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #    "550 5.7.22 No valid author-matched DKIM signature found\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(name = "c550_7_22")]
    pub fn c550_7_22() -> Code {
        code_enhanced(
            550,
            "5.7.22",
            "No valid author-matched DKIM signature found",
        )
        .expect("valid code")
    }

    /// Return a SPF Failure code. (RFC 7208)
    /// Validation failed.
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "550 5.7.23 SPF validation failed" to the client.
    ///         rule "deny with code" || { state::deny(code::c550_7_23()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "550 5.7.23 SPF validation failed\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:6
    #[rhai_fn(name = "c550_7_23")]
    pub fn c550_7_23() -> Code {
        code_enhanced(550, "5.7.23", "SPF validation failed").expect("valid code")
    }

    /// Return a SPF Failure code. (RFC 7208)
    /// Validation error.
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "550 5.7.24 SPF validation error" to the client.
    ///         rule "deny with code" || { state::deny(code::c550_7_24()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "550 5.7.24 SPF validation error\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:7
    #[rhai_fn(name = "c550_7_24")]
    pub fn c550_7_24() -> Code {
        code_enhanced(550, "5.7.24", "SPF validation error").expect("valid code")
    }

    /// Return a reverse DNS Failure code.
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "550 5.7.25 Reverse DNS validation failed" to the client.
    ///         rule "deny with code" || { state::deny(code::c550_7_25()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "550 5.7.25 Reverse DNS validation failed\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:8
    #[rhai_fn(name = "c550_7_25")]
    pub fn c550_7_25() -> Code {
        code_enhanced(550, "5.7.25", "Reverse DNS validation failed").expect("valid code")
    }

    /// Return a multiple authentication failures code.
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "500 5.7.26 Multiple authentication checks failed" to the client.
    ///         rule "deny with code" || { state::deny(code::c500_7_26()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "500 5.7.26 Multiple authentication checks failed\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:9
    #[rhai_fn(name = "c500_7_26")]
    pub fn c550_7_26() -> Code {
        code_enhanced(500, "5.7.26", "Multiple authentication checks failed").expect("valid code")
    }

    /// Return a Null MX cod. (RFC 7505)
    /// The sender address has a null MX record.
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "550 5.7.27 Sender address has null MX" to the client.
    ///         rule "deny with code" || { state::deny(code::c550_7_27()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "550 5.7.27 Sender address has null MX\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:10
    #[rhai_fn(name = "c550_7_27")]
    pub fn c550_7_27() -> Code {
        code_enhanced(550, "5.7.27", "Sender address has null MX").expect("valid code")
    }

    /// Return a Null MX cod. (RFC 7505)
    /// The recipient address has a null MX record.
    ///
    /// # Example
    ///
    /// ```
    /// # // Returning a access denied code in mail stage is stupid, but it works as an example.
    /// # // Could not make it work at the rcpt stage.
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "556 5.1.10 Recipient address has null MX" to the client.
    ///         rule "deny with code" || { state::deny(code::c556_1_10()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "556 5.1.10 Recipient address has null MX\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:11
    #[rhai_fn(name = "c556_1_10")]
    pub fn c556_1_10() -> Code {
        code_enhanced(556, "5.1.10", "Recipient address has null MX").expect("valid code")
    }

    /// Return a greylisting code (<https://www.rfc-editor.org/rfc/rfc6647.html#section-2.1>)
    ///
    /// # Example
    ///
    /// ```
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "451 4.7.1 Sender is not authorized. Please try again." to the client.
    ///         rule "deny with code" || { state::deny(code::c451_7_1()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "451 4.7.1 Sender is not authorized. Please try again.\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:12
    #[rhai_fn(name = "c451_7_1")]
    pub fn greylist() -> Code {
        code_enhanced(451, "4.7.1", "Sender is not authorized. Please try again.")
            .expect("valid code")
    }

    /// Multiple destination domains per transaction is unsupported code.
    ///
    /// # Example
    ///
    /// ```
    /// # // Returning a access denied code in mail stage is stupid, but it works as an example.
    /// # // Could not make it work at the rcpt stage.
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "451 4.3.0 Multiple destination domains per transaction is unsupported. Please try again." to the client.
    ///         rule "deny with code" || { state::deny(code::c451_3_0()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "451 4.3.0 Multiple destination domains per transaction is unsupported. Please try again.\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:13
    #[rhai_fn(name = "c451_3_0")]
    pub fn multi_destination() -> Code {
        code_enhanced(
            451,
            "4.3.0",
            "Multiple destination domains per transaction is unsupported. Please try again.",
        )
        .expect("valid code")
    }

    /// Multiple destination domains per transaction is unsupported code.
    ///
    /// # Example
    ///
    /// ```
    /// # // Returning a access denied code in mail stage is stupid, but it works as an example.
    /// # // Could not make it work at the rcpt stage.
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     mail: [
    ///         // Will send "550 5.1.1 No passing DKIM signature found" to the client.
    ///         rule "deny with code" || { state::deny(code::c550_1_1()) }
    ///     ]
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::ReceiverStatus, Reply, ReplyCode::Enhanced};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::MailFrom].2,
    /// #   ReceiverStatus::Deny(
    /// #     "550 5.1.1 The email account that you tried to reach does not exist. Please try again.\r\n".parse().expect("valid code"),
    /// #   )
    /// # );
    /// ```
    ///
    /// # rhai-autodocs:index:14
    #[rhai_fn(name = "c550_1_1")]
    pub fn unknown_account() -> Code {
        code_enhanced(
            550,
            "5.1.1",
            "The email account that you tried to reach does not exist. Please try again.",
        )
        .expect("valid code")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        assert_eq!(
            code::c554_7_1().to_string(),
            "554 5.7.1 Relay access denied\r\n".to_string()
        );
        assert_eq!(
            code::c550_7_20().to_string(),
            "550 5.7.20 No passing DKIM signature found\r\n".to_string()
        );
        assert_eq!(
            code::c550_7_21().to_string(),
            "550 5.7.21 No acceptable DKIM signature found\r\n".to_string()
        );
        assert_eq!(
            code::c550_7_22().to_string(),
            "550 5.7.22 No valid author-matched DKIM signature found\r\n".to_string()
        );
        assert_eq!(
            code::c550_7_23().to_string(),
            "550 5.7.23 SPF validation failed\r\n".to_string()
        );
        assert_eq!(
            code::c550_7_24().to_string(),
            "550 5.7.24 SPF validation error\r\n".to_string()
        );
        assert_eq!(
            code::c550_7_25().to_string(),
            "550 5.7.25 Reverse DNS validation failed\r\n".to_string()
        );
        assert_eq!(
            code::c550_7_26().to_string(),
            "500 5.7.26 Multiple authentication checks failed\r\n".to_string()
        );
        assert_eq!(
            code::c550_7_27().to_string(),
            "550 5.7.27 Sender address has null MX\r\n".to_string()
        );
        assert_eq!(
            code::c556_1_10().to_string(),
            "556 5.1.10 Recipient address has null MX\r\n".to_string()
        );
        assert_eq!(
            code::greylist().to_string(),
            "451 4.7.1 Sender is not authorized. Please try again.\r\n".to_string()
        );
        assert_eq!(
            code::multi_destination().to_string(),
            "451 4.3.0 Multiple destination domains per transaction is unsupported. Please try again.\r\n".to_string()
        );
        assert_eq!(
            code::unknown_account().to_string(),
            "550 5.1.1 The email account that you tried to reach does not exist. Please try again.\r\n".to_string()
        );
    }
}
