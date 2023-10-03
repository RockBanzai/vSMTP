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
use crate::api::docs::{Ctx, Mail};
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_mail_parser::mail::headers::Header;

pub use message::*;

/// Inspect incoming messages.
#[rhai::plugin::export_module]
mod message {
    use vsmtp_common::stateful_ctx_received::StateError;

    /// Get a copy of the whole email as a string.
    ///
    /// # SMTP stages
    ///
    /// `pre_queue` and onwards.
    ///
    /// # Example
    ///
    /// ```js
    /// let mail = ctx.mail_str;
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, get = "mail_str", return_raw)]
    pub fn mail_str(ctx: &mut Ctx) -> Result<String> {
        ctx.read(|ctx| ctx.get_mail(ToString::to_string).map_err(StateError::into))
    }

    /// Get a reference to the email.
    ///
    /// ```js
    /// let mail_ref = ctx.mail;
    /// ```
    /// # rhai-autodocs:index:2
    #[rhai_fn(global, get = "mail", return_raw)]
    pub fn mail_object(ctx: &mut Ctx) -> Result<Mail> {
        ctx.read(|ctx| ctx.get_mail_arc().map_err(StateError::into))
    }

    /// Return a debug string of the email.
    ///
    /// # rhai-autodocs:index:3
    #[rhai_fn(global, pure)]
    pub fn to_debug(mail: &mut Mail) -> String {
        format!("{mail:?}")
    }

    /// Checks if the message contains a specific header.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to search.
    ///
    /// # SMTP stages
    ///
    /// All of them, although it is most useful in the `pre_queue` stage because the
    /// email is received at this point.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     if ctx.has_header("X-My-Header") {
    ///       state::accept()
    ///     } else {
    ///       state::deny()
    ///     }
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, name = "has_header", return_raw)]
    pub fn has_header(ctx: &mut Ctx, header: &str) -> Result<bool> {
        ctx.read(|ctx| {
            ctx.get_mail(|mail| mail.get_header(header).is_some())
                .map_err(StateError::into)
        })
    }

    /// Count the number of headers with the given name.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to count.
    ///
    /// # Return
    ///
    /// * `number` - the number headers with the same name.
    ///
    /// # SMTP stages
    ///
    /// All of them, although it is most useful in the `pre_queue` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     log("my_queue", "info", `X-My-Header header count: ${ctx.count_header("X-My-Header")}`);
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, name = "count_header", return_raw)]
    pub fn count_header(ctx: &mut Ctx, header: &str) -> Result<rhai::INT> {
        ctx.read(|ctx| {
            ctx.get_mail(|mail| {
                mail.count_header(header.as_ref())
                    .try_into()
                    .map_err::<Box<rhai::EvalAltResult>, _>(|_| "header count overflowed".into())
            })
            .map_err::<Box<rhai::EvalAltResult>, _>(StateError::into)?
        })
    }

    /// Get a specific header from the incoming message.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to get.
    ///
    /// # Return
    ///
    /// * `string`  - the header value if the header was found.
    /// * `()`      - a rhai unit if the header was not found.
    ///
    /// # SMTP stages
    ///
    /// All of them, although it is most useful in the `pre_queue` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     if ctx["X-My-Header"] != "foo" {
    ///       state::deny();
    ///     } else {
    ///       state::accept();
    ///     }
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, index_get, return_raw)]
    pub fn get_header(ctx: &mut Ctx, header: &str) -> Result<rhai::Dynamic> {
        ctx.read(|ctx| {
            ctx.get_mail(|mail| {
                mail.get_header(header)
                    .map_or_else(|| Ok(().into()), |header| Ok(header.body.clone().into()))
            })
            .map_err::<Box<rhai::EvalAltResult>, _>(StateError::into)?
        })
    }

    /// Get a list of all headers.
    ///
    /// # Return
    ///
    /// * `array` - all of the headers found in the message.
    ///
    /// # SMTP stages
    ///
    /// All of them, although it is most useful in the `pre_queue` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_pre_queue (ctx) {
    ///     let headers = ctx.headers;
    ///     // Explore the headers ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:7
    #[rhai_fn(global, get = "headers", return_raw)]
    pub fn get_all_headers(ctx: &mut Ctx) -> Result<rhai::Array> {
        ctx.read(|ctx| {
            ctx.get_mail(|mail| {
                mail.headers
                    .iter()
                    .map(|header| rhai::Dynamic::from(header.to_string()))
                    .collect()
            })
            .map_err(StateError::into)
        })
    }

    /// Get a list of all headers that have the same name.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to search. (optional, if not set, returns every header)
    ///
    /// # Return
    ///
    /// * `array` - all of the headers found in the message that match the given name.
    ///
    /// # SMTP stages
    ///
    /// All of them, although it is most useful in the `pre_queue` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```
    /// fn on_pre_queue (ctx) {
    ///     let headers = ctx.headers("Received");
    ///     // Explore the "Received" headers ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:8
    #[rhai_fn(global, name = "headers", return_raw)]
    pub fn get_all_headers_str(ctx: &mut Ctx, name: &str) -> Result<rhai::Array> {
        ctx.read(|ctx| {
            ctx.get_mail(|mail| {
                mail.get_headers(name)
                    .map(|header| rhai::Dynamic::from(header.to_string()))
                    .collect::<rhai::Array>()
            })
            .map_err(StateError::into)
        })
    }

    /// Get a list of all headers of a specific name with it's name and value
    /// separated by a column.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to search.
    ///
    /// # Return
    ///
    /// * `array` - all header values, or an empty array if the header was not found.
    ///
    /// # SMTP stages
    ///
    /// All of them, although it is most useful in the `pre_queue` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     // Will log "Return-Path: value".
    ///     log("my_queue", "info", ctx.header_untouched("Return-Path"));
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:9
    #[rhai_fn(global, name = "header_untouched", return_raw)]
    pub fn get_header_untouched(ctx: &mut Ctx, name: &str) -> Result<rhai::Array> {
        ctx.read(|ctx| {
            ctx.get_mail(|mail| {
                mail.headers
                    .iter()
                    .filter(|header| header.name.eq_ignore_ascii_case(name))
                    .map(|header| rhai::Dynamic::from(header.to_string()))
                    .collect::<Vec<_>>()
            })
            .map_err(StateError::into)
        })
    }

    /// Add a new header **at the end** of the header list in the message.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to append.
    /// * `value` - the value of the header to append.
    ///
    /// # SMTP stages
    ///
    /// All of them. Even though the email is not received at the current stage,
    /// vsmtp stores new headers and will add them on top of the ones received once
    /// the `pre_queue` stage is reached.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     ctx.append_header("X-My-Header", "foo");
    ///     ctx.append_header("X-My-Header-2", "bar");
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:10
    #[rhai_fn(global, name = "append_header", return_raw)]
    pub fn append_header(ctx: &mut Ctx, name: &str, body: &str) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| mail.append_headers([Header::new(name, body)]))
                .map_err(StateError::into)
        })
    }

    /// Add a new header on top all other headers in the message.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to prepend.
    /// * `value` - the value of the header to prepend.
    ///
    /// # SMTP stages
    ///
    /// All of them. Even though the email is not received at the current stage,
    /// vsmtp stores new headers and will add them on top of the ones received once
    /// the `pre_queue` stage is reached.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     ctx.prepend_header("X-My-Header", "foo");
    ///     ctx.prepend_header("X-My-Header-2", "bar");
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:11
    #[rhai_fn(global, name = "prepend_header", return_raw)]
    pub fn prepend_header(ctx: &mut Ctx, header: &str, value: &str) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| mail.prepend_headers([Header::new(header, value)]))
                .map_err(StateError::into)
        })
    }

    /// Replace an existing header value by a new value, or append a new header
    /// to the message.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to set or add.
    /// * `value` - the value of the header to set or add.
    ///
    /// # SMTP stages
    ///
    /// All of them. Even though the email is not received at the current stage,
    /// vsmtp stores new headers and will add them on top to the ones received once
    /// the `pre_queue` stage is reached.
    ///
    /// Be aware that if you want to set a header value from the original message,
    /// you must use `set_header` in the `pre_queue` stage and onwards.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     ctx["X-My-Header"] = "foo";
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:12
    #[rhai_fn(global, index_set, return_raw)]
    pub fn set_header(ctx: &mut Ctx, header: &str, value: &str) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| mail.set_header(header.as_ref(), value.as_ref()))
                .map_err(StateError::into)
        })
    }

    /// Replace an existing header name by a new value.
    ///
    /// # Args
    ///
    /// * `old_name` - the name of the header to rename.
    /// * `new_name` - the new name of the header.
    ///
    /// # SMTP stages
    ///
    /// All of them, although it is most useful in the `pre_queue` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     ctx.rename_header("X-Subject", "Subject");
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:13
    #[rhai_fn(global, name = "rename_header", return_raw)]
    pub fn rename_header(ctx: &mut Ctx, old_name: &str, new_name: &str) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| mail.rename_header(old_name.as_ref(), new_name.as_ref()))
                .map_err(StateError::into)
        })
    }

    /// Remove an existing header from the message.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to remove.
    ///
    /// # Return
    ///
    /// * a boolean value, true if a header has been removed, false otherwise.
    ///
    /// # SMTP stages
    ///
    /// All of them, although it is most useful in the `pre_queue` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     ctx.remove_header("X-Subject");
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:14
    #[rhai_fn(global, name = "remove_header", return_raw)]
    pub fn remove_header(ctx: &mut Ctx, header: &str) -> Result<bool> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| mail.remove_header(header.as_ref()))
                .map_err(StateError::into)
        })
    }

    /// Change the sender's address in the `From` header of the message.
    ///
    /// # Args
    ///
    /// * `new_addr` - the new sender address to set.
    ///
    /// # SMTP stages
    ///
    /// `pre_queue` and onwards.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     ctx.rewrite_mail_from("john.doe@example.com");
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:15
    #[rhai_fn(global, name = "rewrite_mail_from", return_raw)]
    pub fn rewrite_mail_from_message_str(ctx: &mut Ctx, new_addr: &str) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| {
                mail.rewrite_mail_from(new_addr.as_ref());
            })
            .map_err(StateError::into)
        })
    }

    /// Replace a recipient by an other in the `To` header of the message.
    ///
    /// # Args
    ///
    /// * `old_addr` - the recipient to replace.
    /// * `new_addr` - the new address to use when replacing `old_addr`.
    ///
    /// # SMTP stages
    ///
    /// `pre_queue` and onwards.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     ctx.rewrite_rcpt("john.doe@example.com", "john.mta@example.com");
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:16
    #[rhai_fn(global, name = "rewrite_rcpt", return_raw)]
    pub fn rewrite_rcpt_message_str_str(
        ctx: &mut Ctx,
        old_addr: &str,
        new_addr: &str,
    ) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| {
                mail.rewrite_rcpt(old_addr, new_addr);
            })
            .map_err(StateError::into)
        })
    }

    /// Add a recipient to the `To` header of the message.
    ///
    /// # Args
    ///
    /// * `addr` - the recipient address to add to the `To` header.
    ///
    /// # SMTP stages
    ///
    /// `pre_queue` and onwards.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     ctx.add_rcpt("john.doe@example.com", "john.mta@example.com");
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:17
    #[rhai_fn(global, name = "add_rcpt", return_raw)]
    pub fn add_rcpt_message_str(ctx: &mut Ctx, new_addr: &str) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| {
                mail.add_rcpt(new_addr);
            })
            .map_err(StateError::into)
        })
    }

    /// Remove a recipient from the `To` header of the message.
    ///
    /// # Args
    ///
    /// * `addr` - the recipient to remove to the `To` header.
    ///
    /// # SMTP stages
    ///
    /// `pre_queue` and onwards.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     ctx.remove_rcpt("john.doe@example.com");
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:18
    #[rhai_fn(global, name = "remove_rcpt", return_raw)]
    pub fn remove_rcpt_message_str(ctx: &mut Ctx, addr: &str) -> Result<()> {
        ctx.write(|ctx| ctx.mut_mail(|mail| mail.remove_rcpt(addr)))
            .map_err(StateError::into)
    }

    /// Get the body of the email as a string.
    ///
    /// # SMTP stages
    ///
    /// `pre_queue` and onwards.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_post_queue(ctx) {
    ///     let body = ctx.body;
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:19
    #[rhai_fn(global, get = "body", return_raw)]
    pub fn body_string(ctx: &mut Ctx) -> Result<String> {
        ctx.write(|ctx| ctx.get_mail(|mail| mail.body.to_string()))
            .map_err(StateError::into)
    }
}
