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

pub use mailbox_rhai::*;
use rhai::plugin::{
    Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult, TypeId,
};

/// Rhai wrapper for the Mailbox type, instead of using dynamic
#[derive(Debug, Clone)]
pub enum MailboxInner {
    Regular(vsmtp_common::Mailbox),
    Null,
}

impl std::fmt::Display for MailboxInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                MailboxInner::Regular(r) => r.to_string(),
                MailboxInner::Null => "<>".to_string(),
            }
        )
    }
}

pub fn deserialize_mailbox_opt<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<MailboxInner>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mailbox = <rhai::Dynamic as serde::Deserialize>::deserialize(deserializer)?;

    if mailbox.is_unit() {
        Ok(None)
    } else {
        mailbox
            .try_cast::<MailboxInner>()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("Failed to deserialize mailbox"))
    }
}

/// Mailboxes types and their methods.
#[rhai::export_module]
mod mailbox_rhai {
    /// A recipient received by the "RCPT TO" SMTP command.
    /// Use `ctx.recipients` to get a list of this object.
    ///
    /// # rhai-autodocs:index:1
    pub type Recipient = rhai::Shared<vsmtp_common::Recipient>;

    /// Get a recipient's domain.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_pre_queue(ctx) {
    ///     for rcpt in ctx.recipients {
    ///         if rcpt.domain == "example.com" {
    ///             // ...
    ///         }
    ///     }
    /// }
    /// ```
    /// # rhai-autodocs:index:2
    #[rhai_fn(global, pure, get = "domain")]
    pub fn recipient_domain(ctx: &mut Recipient) -> String {
        ctx.forward_path.domain().to_string()
    }

    /// Get a recipient's address local part.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_pre_queue(ctx) {
    ///     for rcpt in ctx.recipients {
    ///         if rcpt.local_part == "john" {
    ///             // ...
    ///         }
    ///     }
    /// }
    /// ```
    /// # rhai-autodocs:index:3
    #[rhai_fn(global, pure, get = "local_part")]
    pub fn recipient_local_part(ctx: &mut Recipient) -> String {
        ctx.forward_path.local_part().to_string()
    }

    /// Get a recipient's address.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_pre_queue(ctx) {
    ///     for rcpt in ctx.recipients {
    ///         if rcpt.address == "john.doe@example.com" {
    ///             // ...
    ///         }
    ///     }
    /// }
    /// ```
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, pure, get = "address")]
    pub fn recipient_address(ctx: &mut Recipient) -> String {
        ctx.forward_path.to_string()
    }

    #[doc(hidden)]
    #[rhai_fn(global, pure, name = "to_string")]
    pub fn recipient_to_string(ctx: &mut Recipient) -> String {
        recipient_address(ctx)
    }

    /// An email address. Can be null in case of a null reverse path.
    ///
    /// # rhai-autodocs:index:5
    pub type Mailbox = MailboxInner;

    /// Check if an address is null.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_mail_from(ctx) {
    ///     if ctx.sender.is_null() {
    ///         // ...
    ///     }
    /// }
    /// ```
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, pure, name = "is_null")]
    pub fn is_null(mailbox: &mut Mailbox) -> bool {
        matches!(mailbox, Mailbox::Null)
    }

    /// Get the domain of the mailbox.
    /// # rhai-autodocs:index:7
    #[rhai_fn(global, pure, get = "domain")]
    pub fn mailbox_domain(mailbox: &mut Mailbox) -> String {
        match mailbox {
            MailboxInner::Regular(r) => r.domain().to_string(),
            MailboxInner::Null => "".to_string(),
        }
    }

    /// Get the local part of the mailbox.
    /// # rhai-autodocs:index:8
    #[rhai_fn(global, pure, get = "local_part")]
    pub fn mailbox_local_part(mailbox: &mut Mailbox) -> String {
        match mailbox {
            MailboxInner::Regular(r) => r.local_part().to_string(),
            MailboxInner::Null => "".to_string(),
        }
    }

    /// Transform a mailbox object into a string.
    /// # rhai-autodocs:index:9
    #[rhai_fn(global, pure, name = "to_string")]
    pub fn mailbox_to_string(mailbox: &mut Mailbox) -> String {
        mailbox.to_string()
    }
}
