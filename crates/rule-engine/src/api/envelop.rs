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

use super::Result;
use crate::api::docs::Ctx;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_common::delivery_route::DeliveryRoute;
use vsmtp_common::Mailbox;
use vsmtp_protocol::Address;

pub use envelop::*;

/// Build a mailbox and return the appropriate error for the following rhai module.
fn mailbox(address: &str) -> Result<vsmtp_common::Mailbox> {
    Ok(Mailbox(
        <Address as std::str::FromStr>::from_str(address).map_err::<Box<rhai::EvalAltResult>, _>(
            |error| format!("failed to parse address: {error}").into(),
        )?,
    ))
}

/// Functions to inspect and mutate the SMTP envelop.
#[rhai::plugin::export_module]
mod envelop {
    /// Rewrite the sender received from the `MAIL FROM` command.
    ///
    /// # Args
    ///
    /// * `new_addr` - the new string sender address to set.
    ///
    /// # Effective smtp stage
    ///
    /// `mail` and onwards.
    ///
    /// # Examples
    ///
    /// ```
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     preq: [
    ///        action "rewrite envelop 1" || envelop::rewrite_mail_from("unknown@example.com"),
    ///     ]
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(name = "rewrite_mail_from", return_raw, pure)]
    pub fn rewrite_mail_from_envelop_str(ctx: &mut Ctx, new_addr: &str) -> Result<()> {
        let mailbox = mailbox(new_addr)?;

        ctx.write(|ctx| {
            ctx.mut_mail_from()?.reverse_path = Some(mailbox);
            Ok(())
        })
    }

    /// Replace a recipient received by a `RCPT TO` command.
    ///
    /// # Args
    ///
    /// * `old_addr` - the recipient to replace.
    /// * `new_addr` - the new address to use when replacing `old_addr`.
    ///
    /// # Effective smtp stage
    ///
    /// `rcpt` and onwards.
    ///
    /// # Examples
    ///
    /// ```
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     preq: [
    ///        action "rewrite envelop" || envelop::rewrite_rcpt("john.doe@example.com", "john.main@example.com"),
    ///     ]
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(name = "rewrite_rcpt", return_raw, pure)]
    pub fn rewrite_rcpt_str_str(ctx: &mut Ctx, old_addr: &str, new_addr: &str) -> Result<()> {
        let old_addr = mailbox(old_addr)?;
        let new_addr = mailbox(new_addr)?;

        ctx.write(|ctx| {
            ctx.mut_rcpt_to()?.rewrite_recipient(&old_addr, new_addr);
            Ok(())
        })
    }

    /// Add a new recipient to the envelop. Note that this does not add
    /// the recipient to the `To` header. Use `msg::add_rcpt` for that.
    ///
    /// # Args
    ///
    /// * `rcpt` - the new recipient to add.
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
    ///     connect: [
    ///        // always deliver a copy of the message to "john.doe@example.com".
    ///        action "rewrite envelop" || envelop::add_rcpt("john.doe@example.com"),
    ///     ]
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:3
    #[rhai_fn(name = "add_rcpt", return_raw, pure)]
    pub fn add_rcpt_envelop_str(ctx: &mut Ctx, new_addr: &str) -> Result<()> {
        let new_addr = mailbox(new_addr)?;

        ctx.write(|ctx| {
            ctx.mut_rcpt_to()?
                .add_recipient_with_route(new_addr, DeliveryRoute::Basic);
            Ok(())
        })
    }

    /// Alias for `envelop::add_rcpt`.
    ///
    /// # rhai-autodocs:index:4
    #[rhai_fn(name = "bcc", return_raw)]
    pub fn bcc_str(ctx: &mut Ctx, new_addr: &str) -> Result<()> {
        super::add_rcpt_envelop_str(ctx, new_addr)
    }

    /// Remove a recipient from the envelop. Note that this does not remove
    /// the recipient from the `To` header. Use `msg::remove_rcpt` for that.
    ///
    /// # Args
    ///
    /// * `rcpt` - the recipient to remove.
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
    ///     preq: [
    ///        // never deliver to "john.doe@example.com".
    ///        action "rewrite envelop 1" || envelop::remove_rcpt("john.doe@example.com"),
    ///        action "rewrite envelop 2" || envelop::remove_rcpt(address("john.doe@example.com")),
    ///     ]
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(name = "remove_rcpt", return_raw)]
    pub fn remove_rcpt_envelop_str(ctx: &mut Ctx, addr: &str) -> Result<()> {
        let addr = mailbox(addr)?;

        ctx.write(|ctx| {
            ctx.mut_rcpt_to()?.remove_recipient(&addr);
            Ok(())
        })
    }
}
