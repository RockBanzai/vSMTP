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

pub use mail_context::*;

// TODO: handle error when the context is not in the expected stage.

/// Inspect the transaction context.
#[rhai::plugin::export_module]
mod mail_context {
    use vsmtp_common::stateful_ctx_received::StatefulCtxReceived;

    /// Set the routing path of a recipient, mostly used to select which forwarding service to sent
    /// the email to before delivery.
    ///
    /// # Args
    ///
    /// * `rcpt` - The selected recipient. (use a for loop with `ctx.recipients`)
    /// * `path` - The routing path to use.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_pre_queue(ctx) {
    ///     for i in ctx.recipients {
    ///         // When a server with the `config.service` field is set to "my-server".
    ///         ctx.set_routing_path(i, "forward.my-server");
    ///     }
    ///     status::next()
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, return_raw, pure)]
    #[tracing::instrument(skip(rcpt), fields(rcpt = %rcpt.forward_path))]
    pub fn set_routing_path(
        ctx: &mut Ctx,
        rcpt: rhai::Shared<vsmtp_common::Recipient>,
        path: &str,
    ) -> Result<()> {
        let path = path
            .parse::<vsmtp_common::delivery_route::DeliveryRoute>()
            .map_err(|e| e.to_string())?;

        ctx.write(|ctx| {
            let map = &mut ctx.mut_rcpt_to().unwrap().recipient;

            for (previous_routing_key, r, idx) in map
                .iter_mut()
                .filter_map(|(k, r)| r.iter().position(|i| *i == *rcpt).map(|idx| (k, r, idx)))
            {
                tracing::debug!(?previous_routing_key, "Recipient already exists, removing");
                r.remove(idx);
            }

            let rcpt = rcpt.as_ref().clone();
            if let Some(values) = map.get_mut(&path) {
                tracing::debug!("Found pre-existing routing key, appending");
                values.push(rcpt);
            } else {
                tracing::debug!("No pre-existing routing key found, creating entry");
                map.insert(path, vec![rcpt]);
            }

            Ok(())
        })
    }

    /// Get the address of the client.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Return
    ///
    /// * `string` - the client's address with the `ip:port` format.
    ///
    /// # Examples
    ///
    ///```js
    /// let client_address = ctx.client_address;
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(global, get = "client_address")]
    pub fn client_address(ctx: &mut Ctx) -> String {
        ctx.read(|ctx| ctx.get_connect().client_addr.to_string())
    }

    /// Get the ip address of the client.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Return
    ///
    /// * `string` - the client's ip address.
    ///
    /// # Example
    ///
    ///```js
    /// let client_ip = ctx.client_ip;
    /// ```
    ///
    /// # rhai-autodocs:index:3
    #[rhai_fn(global, get = "client_ip")]
    pub fn client_ip(ctx: &mut Ctx) -> String {
        ctx.read(|ctx| ctx.get_connect().client_addr.ip().to_string())
    }

    /// Get the ip port of the client.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Return
    ///
    /// * `int` - the client's port.
    ///
    /// # Example
    ///
    ///```js
    /// let client_port = ctx.client_port;
    /// ```
    ///
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, get = "client_port")]
    pub fn client_port(ctx: &mut Ctx) -> rhai::INT {
        ctx.read(|ctx| ctx.get_connect().client_addr.port() as rhai::INT)
    }

    /// Get the full server address.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Return
    ///
    /// * `string` - the server's address with the `ip:port` format.
    ///
    /// # Example
    ///
    ///```js
    /// let server_address = ctx.server_address;
    /// ```
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, get = "server_address")]
    pub fn server_address(ctx: &mut Ctx) -> String {
        ctx.read(|ctx| ctx.get_connect().server_addr.to_string())
    }

    /// Get the server's ip.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Return
    ///
    /// * `string` - the server's ip.
    ///
    /// # Example
    ///
    ///```js
    /// let server_ip = ctx.server_ip;
    /// ```
    ///
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, get = "server_ip")]
    pub fn server_ip(ctx: &mut Ctx) -> String {
        ctx.read(|ctx| ctx.get_connect().server_addr.ip().to_string())
    }

    /// Get the server's port.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Return
    ///
    /// * `string` - the server's port.
    ///
    /// # Example
    ///
    ///```js
    /// let server_port = ctx.server_port;
    /// ```
    ///
    /// # rhai-autodocs:index:7
    #[rhai_fn(global, get = "server_port")]
    pub fn server_port(ctx: &mut Ctx) -> rhai::INT {
        ctx.read(|ctx| ctx.get_connect().server_addr.port() as rhai::INT)
    }

    /// Get a the timestamp of the client's connection time.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Return
    ///
    /// * `timestamp` - the connection timestamp of the client.
    ///
    /// # Example
    ///
    ///```js
    /// let connection_timestamp = ctx.connection_timestamp;
    /// ```
    ///
    /// # rhai-autodocs:index:8
    #[rhai_fn(global, get = "connection_timestamp")]
    pub fn connection_timestamp(ctx: &mut Ctx) -> vsmtp_common::time::OffsetDateTime {
        ctx.read(|ctx| ctx.get_connect().connect_timestamp)
    }

    /// Get the name of the server.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Return
    ///
    /// * `string` - the name of the server.
    ///
    /// # Example
    ///
    ///```js
    /// let server_name = ctx.server_name;
    /// ```
    ///
    /// # rhai-autodocs:index:9
    #[rhai_fn(global, get = "server_name")]
    pub fn server_name(ctx: &mut Ctx) -> String {
        ctx.read(|ctx| ctx.get_connect().server_name.to_string())
    }

    /// Has the connection been secured under the encryption protocol SSL/TLS.
    ///
    /// # SMTP stages
    ///
    /// all of them.
    ///
    /// # Return
    ///
    /// * `bool` - `true` if the connection is secured, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```js
    /// log("my_queue", "debug", `Transaction is ${if ctx::is_secured() { "secured" } else { "unsecured" }}.`);
    /// ```
    ///
    /// # rhai-autodocs:index:10
    #[rhai_fn(global, name = "is_secured")]
    pub fn is_secured(ctx: &mut Ctx) -> bool {
        ctx.read(StatefulCtxReceived::is_secured)
    }

    /// Get the value of the `HELO/EHLO` command sent by the client.
    ///
    /// # SMTP stages
    ///
    /// `helo` and onwards.
    ///
    /// # Return
    ///
    /// * `string` - the value of the `HELO/EHLO` command.
    ///
    /// # Examples
    ///
    /// ```js
    /// log("my_queue", "info", `helo value: ${ctx.helo}`);
    /// ```
    ///
    /// # rhai-autodocs:index:11
    #[rhai_fn(global, get = "helo", return_raw)]
    pub fn helo(ctx: &mut Ctx) -> Result<String> {
        ctx.read(|ctx| ctx.get_helo().map(|helo| helo.client_name.to_string()))
            .map_err(|e| e.to_string().into())
    }

    /// Get the value of the `MAIL FROM` command sent by the client.
    ///
    /// # SMTP stages
    ///
    /// `mail` and onwards.
    ///
    /// # Return
    ///
    /// `String` or `"<>"` (for null reverse path))
    ///
    /// # Examples
    ///
    /// ```js
    /// log("my_queue", "info", `sender: ${ctx.sender}`);
    /// ```
    ///
    /// # rhai-autodocs:index:12
    #[rhai_fn(global, get = "sender", return_raw)]
    pub fn sender(ctx: &mut Ctx) -> Result<rhai::Dynamic> {
        ctx.read(|ctx| {
            Ok(ctx
                .get_mail_from()
                .unwrap()
                .reverse_path
                .clone()
                .map_or_else(|| "<>".into(), rhai::Dynamic::from))
        })
    }

    /// Get the list of recipients received by the client.
    ///
    /// # SMTP stages
    ///
    /// `rcpt` and onwards. Note that you will not have all recipients received
    /// all at once in the `rcpt` stage. It is better to use this function
    /// in the later stages.
    ///
    /// # Return
    ///
    /// * `Array of addresses` - the list containing all recipients.
    ///
    /// # Examples
    ///
    /// ```js
    /// log("my_queue", "info", `recipients: ${ctx.recipients}`);
    /// ```
    ///
    /// # rhai-autodocs:index:13
    #[rhai_fn(global, return_raw, get = "recipients")]
    pub fn recipients(ctx: &mut Ctx) -> Result<rhai::Array> {
        ctx.read(|ctx| {
            Ok(ctx
                .get_rcpt_to()
                .unwrap()
                .recipient
                .values()
                .flat_map(|i| i.iter().cloned())
                .map(rhai::Shared::new)
                .map(rhai::Dynamic::from)
                .collect::<rhai::Array>())
        })
    }

    // TODO: How do we get the last rcpt ?
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
    /// # rhai-autodocs:index:14
    #[rhai_fn(global, pure, get = "domain")]
    pub fn recipient_domain(ctx: &mut rhai::Shared<vsmtp_common::Recipient>) -> String {
        ctx.forward_path.domain().to_string()
    }

    #[doc(hidden)]
    #[rhai_fn(global, pure, get = "domain")]
    pub fn mailbox_domain(ctx: &mut vsmtp_common::Mailbox) -> String {
        ctx.domain().to_string()
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
    /// # rhai-autodocs:index:15
    #[rhai_fn(global, pure, get = "local_part")]
    pub fn recipient_local_part(ctx: &mut rhai::Shared<vsmtp_common::Recipient>) -> String {
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
    /// # rhai-autodocs:index:16
    #[rhai_fn(global, pure, get = "address")]
    pub fn recipient_address(ctx: &mut rhai::Shared<vsmtp_common::Recipient>) -> String {
        ctx.forward_path.to_string()
    }

    /// Check if the address is null.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_pre_queue(ctx) {
    ///     for rcpt in ctx.recipients {
    ///         if rcpt.address.is_null {
    ///             // ...
    ///         }
    ///     }
    /// }
    /// ```
    /// # rhai-autodocs:index:17
    #[rhai_fn(global, pure, get = "is_null")]
    pub fn is_null(mailbox: &mut rhai::Dynamic) -> bool {
        !mailbox.is::<vsmtp_common::Mailbox>()
    }

    /// Get the time of reception of the email.
    ///
    /// # SMTP stages
    ///
    /// `pre_queue` and onwards.
    ///
    /// # Return
    ///
    /// * `string` - the timestamp.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_pre_queue(ctx) {
    ///     log("my_queue", "info", `time of reception: ${ctx.mail_timestamp}`);
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:18
    #[rhai_fn(global, get = "mail_timestamp", return_raw)]
    pub fn mail_timestamp(ctx: &mut Ctx) -> Result<vsmtp_common::time::OffsetDateTime> {
        ctx.read(|ctx| Ok(ctx.get_mail_from().unwrap().mail_timestamp))
    }

    /// Get the unique id of the received message.
    ///
    /// # SMTP stages
    ///
    /// `pre_queue` and onwards.
    ///
    /// # Return
    ///
    /// * `string` - the message id.
    ///
    /// # Examples
    ///
    /// ```js
    /// fn on_pre_queue(ctx) {
    ///     log("my_queue", "info", `message id: ${ctx.message_id}`);
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:19
    #[rhai_fn(global, get = "message_id", return_raw)]
    pub fn message_id(ctx: &mut Ctx) -> Result<String> {
        ctx.read(|ctx| Ok(ctx.get_mail_from().unwrap().message_uuid.to_string()))
    }

    /// Transform the context to a debug string.
    /// # rhai-autodocs:index:20
    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn to_debug(ctx: &mut Ctx) -> String {
        format!("{ctx:?}")
    }
}
