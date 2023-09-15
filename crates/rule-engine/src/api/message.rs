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

use super::{Result, State};
use vsmtp_common::stateful_ctx_received::StatefulCtxReceived;
use vsmtp_mail_parser::mail::headers::Header;
use vsmtp_mail_parser::Mail;

pub use message::*;

/// Inspect incoming messages.
#[rhai::plugin::export_module]
mod message {
    use vsmtp_common::stateful_ctx_received::StateError;

    /// Get a copy of the whole email as a string.
    ///
    /// # Effective smtp stage
    ///
    /// `preq` and onwards.
    ///
    /// # Example
    ///
    /// ```
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     postq: [
    ///        action "display email content" || log("trace", `email content: ${msg::mail()}`),
    ///     ]
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, get = "mail_str", return_raw)]
    pub fn mail_str(ctx: &mut State<StatefulCtxReceived>) -> Result<String> {
        ctx.read(|ctx| ctx.get_mail(ToString::to_string).map_err(StateError::into))
    }

    /// Get a reference to the email.
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(global, get = "mail", return_raw)]
    pub fn mail_object(
        ctx: &mut State<StatefulCtxReceived>,
    ) -> Result<std::sync::Arc<std::sync::RwLock<Mail>>> {
        ctx.read(|ctx| ctx.get_mail_arc().map_err(StateError::into))
    }

    /// Return a debug string of the email.
    ///
    /// # rhai-autodocs:index:3
    #[rhai_fn(global, pure)]
    pub fn to_debug(mail: &mut std::sync::Arc<std::sync::RwLock<Mail>>) -> String {
        format!("{mail:?}")
    }

    /// Checks if the message contains a specific header.
    ///
    /// # Args
    ///
    /// * `header` - the name of the header to search.
    ///
    /// # Effective smtp stage
    ///
    /// All of them, although it is most useful in the `preq` stage because the
    /// email is received at this point.
    ///
    /// # Examples
    ///
    /// ```
    /// // Message example.
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(concat!(
    /// "X-My-Header: foo\r\n",
    /// "Subject: Unit test are cool\r\n",
    /// "\r\n",
    /// "Hello world!\r\n",
    /// # )).unwrap();
    /// # let rules = r#"
    /// #{
    ///   preq: [
    ///     rule "check if header exists" || {
    ///       if msg::has_header("X-My-Header") && msg::has_header(identifier("Subject")) {
    ///         state::accept();
    ///       } else {
    ///         state::deny();
    ///       }
    ///     }
    ///   ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), Some(msg));
    /// # use vsmtp_common::{status::Status};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::PreQ].2, Status::Accept("250 Ok".parse::<vsmtp_common::Reply>().unwrap()));
    /// ```
    ///
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, name = "has_header", return_raw)]
    pub fn has_header(ctx: &mut State<StatefulCtxReceived>, header: &str) -> Result<bool> {
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
    /// # Effective smtp stage
    ///
    /// All of them, although it is most useful in the `preq` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(concat!(
    /// "X-My-Header: foo\r\n",
    /// "X-My-Header: bar\r\n",
    /// "X-My-Header: baz\r\n",
    /// "Subject: Unit test are cool\r\n",
    /// "\r\n",
    /// "Hello world!\r\n",
    /// # )).unwrap();
    /// # let rules = r#"
    /// #{
    ///   preq: [
    ///     rule "count_header" || {
    ///       state::accept(`250 count is ${msg::count_header("X-My-Header")} and ${msg::count_header(identifier("Subject"))}`);
    ///     }
    ///   ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), Some(msg));
    /// # use vsmtp_common::{status::Status, Reply, ReplyCode::Code};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::PreQ].2, Status::Accept(
    /// #  "250 count is 3 and 1\r\n".parse().unwrap()
    /// # ));
    /// ```
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, name = "count_header", return_raw)]
    pub fn count_header(ctx: &mut State<StatefulCtxReceived>, header: &str) -> Result<rhai::INT> {
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
    /// # Effective smtp stage
    ///
    /// All of them, although it is most useful in the `preq` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```
    /// # let msg = r#"
    /// X-My-Header: 250 foo
    /// Subject: Unit test are cool
    ///
    /// Hello world!
    /// # "#
    /// ; // .eml ends here
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(msg[1..].replace("\n", "\r\n").as_str()).unwrap();
    ///
    /// let rules = r#"
    /// #{
    ///   preq: [
    ///     rule "get_header" || {
    ///       if msg::get_header("X-My-Header") != "250 foo"
    ///         || msg::get_header(identifier("Subject")) != "Unit test are cool" {
    ///         state::deny();
    ///       } else {
    ///         state::accept(`${msg::get_header("X-My-Header")} ${msg::get_header(identifier("Subject"))}`);
    ///       }
    ///     }
    ///   ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), Some(msg));
    /// # use vsmtp_common::{status::Status, Reply, ReplyCode::Code};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::PreQ].2, Status::Accept(
    /// #  "250 foo Unit test are cool\r\n".parse().unwrap()
    /// # ));
    /// ```
    ///
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, index_get, return_raw)]
    pub fn get_header(ctx: &mut State<StatefulCtxReceived>, header: &str) -> Result<rhai::Dynamic> {
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
    /// # Args
    ///
    /// * `header` - the name of the header to search. (optional, if not set, returns every header)
    ///
    /// # Return
    ///
    /// * `array` - all of the headers found in the message.
    ///
    /// # Effective smtp stage
    ///
    /// All of them, although it is most useful in the `preq` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```
    /// # let msg = r#"
    /// X-My-Header: 250 foo
    /// Subject: Unit test are cool
    ///
    /// Hello world!
    /// # "#
    /// ; // .eml ends here
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(msg[1..].replace("\n", "\r\n").as_str()).unwrap();
    ///
    /// # let states = vsmtp_test::rhai::run_with_msg(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///   preq: [
    ///     rule "display headers" || {
    ///         log("info", `all headers: ${msg::get_all_headers()}`);
    ///         log("info", `all "Return-Path" headers: ${msg::get_all_headers("Return-Path")}`);
    ///     }
    ///   ]
    /// }
    /// # "#)?.build()), Some(msg));
    /// ```
    ///
    /// # rhai-autodocs:index:7
    #[rhai_fn(global, get = "headers", return_raw)]
    pub fn get_all_headers(ctx: &mut State<StatefulCtxReceived>) -> Result<rhai::Array> {
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

    #[doc(hidden)]
    #[rhai_fn(global, name = "headers", return_raw)]
    pub fn get_all_headers_str(
        ctx: &mut State<StatefulCtxReceived>,
        name: &str,
    ) -> Result<rhai::Array> {
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
    /// # Effective smtp stage
    ///
    /// All of them, although it is most useful in the `preq` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```
    /// # let msg = r#"
    /// X-My-Header: 250 foo
    /// Subject: Unit test are cool
    ///
    /// Hello world!
    /// # "#
    /// ; // .eml ends here
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(msg[1..].replace("\n", "\r\n").as_str()).unwrap();
    ///
    /// # let states = vsmtp_test::rhai::run_with_msg(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     postq: [
    ///         action "display return path" || {
    ///             // Will display "Return-Path: value".
    ///             log("info", msg::get_header_untouched("Return-Path"));
    ///         }
    ///     ],
    /// }
    /// # "#)?.build()), Some(msg));
    /// ```
    ///
    /// # rhai-autodocs:index:8
    #[rhai_fn(global, name = "header_untouched", return_raw)]
    pub fn get_header_untouched(
        ctx: &mut State<StatefulCtxReceived>,
        name: &str,
    ) -> Result<rhai::Array> {
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
    /// # Effective smtp stage
    ///
    /// All of them. Even though the email is not received at the current stage,
    /// vsmtp stores new headers and will add them on top of the ones received once
    /// the `preq` stage is reached.
    ///
    /// # Examples
    ///
    /// ```
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(concat!(
    /// "X-My-Header: 250 foo\r\n",
    /// "Subject: Unit test are cool\r\n",
    /// "\r\n",
    /// "Hello world!\r\n",
    /// # )).unwrap();
    /// # let rules = r#"
    /// #{
    ///   preq: [
    ///     rule "append_header" || {
    ///       msg::append_header("X-My-Header-2", "bar");
    ///       msg::append_header("X-My-Header-3", identifier("baz"));
    ///     }
    ///   ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), Some(msg));
    /// # assert_eq!(*states[&vsmtp_rule_engine::ExecutionStage::PreQ].1.inner().raw_headers(), vec![
    /// #   "X-My-Header: 250 foo\r\n".to_string(),
    /// #   "Subject: Unit test are cool\r\n".to_string(),
    /// #   "X-My-Header-2: bar\r\n".to_string(),
    /// #   "X-My-Header-3: baz\r\n".to_string(),
    /// # ]);
    /// ```
    ///
    /// # rhai-autodocs:index:9
    #[rhai_fn(global, name = "append_header", return_raw)]
    pub fn append_header(
        ctx: &mut State<StatefulCtxReceived>,
        name: &str,
        body: &str,
    ) -> Result<()> {
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
    /// # Effective smtp stage
    ///
    /// All of them. Even though the email is not received at the current stage,
    /// vsmtp stores new headers and will add them on top of the ones received once
    /// the `preq` stage is reached.
    ///
    /// # Examples
    ///
    /// ```
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(concat!(
    /// "X-My-Header: 250 foo\r\n",
    /// "Subject: Unit test are cool\r\n",
    /// "\r\n",
    /// "Hello world!\r\n",
    /// # )).unwrap();
    /// # let rules = r#"
    /// #{
    ///   preq: [
    ///     rule "prepend_header" || {
    ///       msg::prepend_header("X-My-Header-2", "bar");
    ///       msg::prepend_header("X-My-Header-3", identifier("baz"));
    ///     }
    ///   ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), Some(msg));
    /// # assert_eq!(*states[&vsmtp_rule_engine::ExecutionStage::PreQ].1.inner().raw_headers(), vec![
    /// #   "X-My-Header-3: baz\r\n".to_string(),
    /// #   "X-My-Header-2: bar\r\n".to_string(),
    /// #   "X-My-Header: 250 foo\r\n".to_string(),
    /// #   "Subject: Unit test are cool\r\n".to_string(),
    /// # ]);
    /// ```
    ///
    /// # rhai-autodocs:index:10
    #[rhai_fn(global, name = "prepend_header", return_raw)]
    pub fn prepend_header(
        ctx: &mut State<StatefulCtxReceived>,
        header: &str,
        value: &str,
    ) -> Result<()> {
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
    /// # Effective smtp stage
    ///
    /// All of them. Even though the email is not received at the current stage,
    /// vsmtp stores new headers and will add them on top to the ones received once
    /// the `preq` stage is reached.
    ///
    /// Be aware that if you want to set a header value from the original message,
    /// you must use `set_header` in the `preq` stage and onwards.
    ///
    /// # Examples
    ///
    /// ```
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(concat!(
    /// "Subject: The initial header value\r\n",
    /// "\r\n",
    /// "Hello world!\r\n",
    /// # )).unwrap();
    /// # let rules = r#"
    /// #{
    ///   preq: [
    ///     rule "set_header" || {
    ///       msg::set_header("Subject", "The header value has been updated");
    ///       msg::set_header("Subject", identifier("The header value has been updated again"));
    ///       state::accept(`250 ${msg::get_header("Subject")}`);
    ///     }
    ///   ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), Some(msg));
    /// # use vsmtp_common::{status::Status, Reply, ReplyCode::Code};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::PreQ].2, Status::Accept(
    /// #  "250 The header value has been updated again\r\n".parse().unwrap()
    /// # ));
    /// ```
    ///
    /// # rhai-autodocs:index:11
    #[rhai_fn(global, index_set, return_raw)]
    pub fn set_header(
        ctx: &mut State<StatefulCtxReceived>,
        header: &str,
        value: &str,
    ) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| mail.set_header(header.as_ref(), value.as_ref()))
                .map_err(StateError::into)
        })
    }

    /// Replace an existing header name by a new value.
    ///
    /// # Args
    ///
    /// * `old` - the name of the header to rename.
    /// * `new` - the new new of the header.
    ///
    /// # Effective smtp stage
    ///
    /// All of them, although it is most useful in the `preq` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(concat!(
    /// "Subject: The initial header value\r\n",
    /// "\r\n",
    /// "Hello world!\r\n",
    /// # )).unwrap();
    ///
    /// # let rules = r#"
    /// #{
    ///   preq: [
    ///     rule "rename_header" || {
    ///       msg::rename_header("Subject", "bob");
    ///       if msg::has_header("Subject") { return state::deny(); }
    ///
    ///       msg::rename_header("bob", identifier("Subject"));
    ///       if msg::has_header("bob") { return state::deny(); }
    ///
    ///       msg::rename_header(identifier("Subject"), "foo");
    ///       if msg::has_header("Subject") { return state::deny(); }
    ///
    ///       msg::rename_header(identifier("foo"), identifier("Subject"));
    ///       if msg::has_header("foo") { return state::deny(); }
    ///
    ///       state::accept(`250 ${msg::get_header("Subject")}`);
    ///     }
    ///   ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), Some(msg));
    /// # use vsmtp_common::{status::Status, Reply, ReplyCode::Code};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::PreQ].2, Status::Accept(
    /// #  "250 The initial header value\r\n".parse().unwrap()
    /// # ));
    /// ```
    ///
    /// # rhai-autodocs:index:12
    #[rhai_fn(global, name = "rename_header", return_raw)]
    pub fn rename_header(ctx: &mut State<StatefulCtxReceived>, old: &str, new: &str) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_mail(|mail| mail.rename_header(old.as_ref(), new.as_ref()))
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
    /// # Effective smtp stage
    ///
    /// All of them, although it is most useful in the `preq` stage because this
    /// is when the email body is received.
    ///
    /// # Examples
    ///
    /// ```
    /// # let msg = vsmtp_mail_parser::MessageBody::try_from(concat!(
    /// "Subject: The initial header value\r\n",
    /// "\r\n",
    /// "Hello world!\r\n",
    /// # )).unwrap();
    /// # let rules = r#"
    /// #{
    ///   preq: [
    ///     rule "remove_header" || {
    ///       msg::rm_header("Subject");
    ///       if msg::has_header("Subject") { return state::deny(); }
    ///
    ///       msg::prepend_header("Subject-2", "Rust is good");
    ///       msg::rm_header(identifier("Subject-2"));
    ///
    ///       msg::prepend_header("Subject-3", "Rust is good !!!!!");
    ///
    ///       state::accept(`250 ${msg::get_header("Subject-3")}`);
    ///     }
    ///   ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), Some(msg));
    /// # use vsmtp_common::{status::Status, Reply, ReplyCode::Code};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::PreQ].2, Status::Accept(
    /// #  "250 Rust is good !!!!!\r\n".parse().unwrap()
    /// # ));
    /// ```
    ///
    /// # rhai-autodocs:index:13
    #[rhai_fn(global, name = "rm_header", return_raw)]
    pub fn remove_header(ctx: &mut State<StatefulCtxReceived>, header: &str) -> Result<bool> {
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
    /// # Effective smtp stage
    ///
    /// `preq` and onwards.
    ///
    /// # Examples
    ///
    ///```
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     preq: [
    ///        action "replace sender" || msg::rw_mail_from("john.server@example.com"),
    ///     ]
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:14
    #[rhai_fn(global, name = "rw_mail_from", return_raw)]
    pub fn rewrite_mail_from_message_str(
        ctx: &mut State<StatefulCtxReceived>,
        new_addr: &str,
    ) -> Result<()> {
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
    /// # Effective smtp stage
    ///
    /// `preq` and onwards.
    ///
    /// # Examples
    ///
    /// ```
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     preq: [
    ///        action "rewrite recipient" || msg::rw_rcpt("john.doe@example.com", "john-mta@example.com"),
    ///     ]
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:15
    #[rhai_fn(global, name = "rw_rcpt", return_raw)]
    pub fn rewrite_rcpt_message_str_str(
        ctx: &mut State<StatefulCtxReceived>,
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
    /// # Effective smtp stage
    ///
    /// `preq` and onwards.
    ///
    /// # Examples
    ///
    /// ```
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     preq: [
    ///        action "update recipients" || msg::add_rcpt("john.doe@example.com"),
    ///     ]
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:16
    #[rhai_fn(global, name = "add_rcpt", return_raw)]
    pub fn add_rcpt_message_str(
        ctx: &mut State<StatefulCtxReceived>,
        new_addr: &str,
    ) -> Result<()> {
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
    /// # Effective smtp stage
    ///
    /// `preq` and onwards.
    ///
    /// # Examples
    ///
    /// ```
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///     preq: [
    ///        action "update recipients" || msg::rm_rcpt("john.doe@example.com"),
    ///     ]
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:17
    #[rhai_fn(global, name = "rm_rcpt", return_raw)]
    pub fn remove_rcpt_message_str(ctx: &mut State<StatefulCtxReceived>, addr: &str) -> Result<()> {
        ctx.write(|ctx| ctx.mut_mail(|mail| mail.remove_rcpt(addr)))
            .map_err(StateError::into)
    }

    /// Get the body of the email as a string.
    ///
    /// # Effective smtp stage
    ///
    /// `preq` and onwards.
    ///
    /// # Examples
    ///
    /// ```
    /// TODO:
    /// ```
    /// # rhai-autodocs:index:18
    #[rhai_fn(global, get = "body", return_raw)]
    pub fn body_string(ctx: &mut State<StatefulCtxReceived>) -> Result<String> {
        ctx.write(|ctx| ctx.get_mail(|mail| mail.body.to_string()))
            .map_err(StateError::into)
    }
}
