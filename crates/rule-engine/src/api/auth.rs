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

use crate::api::docs::Ctx;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult,
    TypeId,
};
use std::sync::Arc;
use vsmtp_auth::{dkim::DkimVerificationResult, dmarc::Dmarc, iprev, spf};
use vsmtp_common::stateful_ctx_received::{StateError, StatefulCtxReceived};
use vsmtp_mail_parser::mail::headers::Header;

struct AuthMechanism {
    iprev: Option<iprev::IpRevResult>,
    spf_helo: Option<Arc<spf::Result>>,
    spf_mail_from: Option<Arc<spf::Result>>,
    dkim: Option<Arc<Vec<DkimVerificationResult>>>,
    dmarc: Option<Arc<Dmarc>>,
}

impl From<&StatefulCtxReceived> for AuthMechanism {
    fn from(value: &StatefulCtxReceived) -> Self {
        let iprev = value.get_connect().iprev.clone();
        let spf_helo = value
            .get_helo()
            .ok()
            .and_then(|helo| helo.spf_helo_identity.clone());
        let spf_mail_from = value
            .get_mail_from()
            .ok()
            .and_then(|mail_from| mail_from.spf_mail_from_identity.clone());
        let dkim = value
            .get_complete()
            .ok()
            .and_then(|complete| complete.dkim.clone());
        let dmarc = value
            .get_complete()
            .ok()
            .and_then(|complete| complete.dmarc.clone());

        Self {
            iprev,
            spf_helo,
            spf_mail_from,
            dkim,
            dmarc,
        }
    }
}

impl AuthMechanism {
    fn make_header(
        Self {
            iprev,
            spf_helo,
            spf_mail_from,
            dkim,
            dmarc,
        }: Self,
        prefix: String,
    ) -> String {
        if iprev.is_some()
            || spf_helo.is_some()
            || spf_mail_from.is_some()
            || dkim.is_some()
            || dmarc.is_some()
        {
            let iprev =
                iprev.map(|iprev| format!("\tiprev={} policy.iprev={};", iprev.value, iprev.ip));

            #[allow(clippy::option_if_let_else)]
            let spf_helo = spf_helo.map(|spf_helo| {
                format!(
                    "\tspf={} {};",
                    spf_helo.value,
                    match &spf_helo.domain {
                        Some(domain) => format!("smtp.helo={domain}"),
                        None => "(helo is an ip)".to_string(),
                    }
                )
            });

            #[allow(clippy::option_if_let_else)]
            let spf_mail_from = spf_mail_from.map(|spf_mail_from| {
                format!(
                    "\tspf={} {};",
                    spf_mail_from.value,
                    match &spf_mail_from.domain {
                        Some(domain) => format!("smtp.mailfrom={domain}"),
                        None => "(helo is an ip)".to_string(),
                    }
                )
            });

            let dkim = dkim.map(|dkim| {
                dkim.iter().map(|dkim| {
                    #[allow(clippy::option_if_let_else)]
                    match &dkim.signature {
                        Some(signature) => format!(
                            "\tdkim={v} header.d={d} header.i={i} header.a={a} header.s={s} header.b={b};",
                                v = dkim.value,
                                d = signature.sdid,
                                i = signature.auid,
                                a = signature.signing_algorithm,
                                s = signature.selector,
                                b = &signature.signature[..=8]
                        ),
                        None => format!(
                            "\tdkim={v}",
                            v = dkim.value,
                        )
                    }
                })
                .collect::<Vec<_>>()
            });

            let dmarc =
                dmarc.map(|dmarc| format!("\tdmarc={} header.from={};", dmarc.value, dmarc.domain));

            std::iter::once(prefix)
                .chain(iprev)
                .chain(spf_helo)
                .chain(spf_mail_from)
                .chain(dkim.map(Vec::into_iter).unwrap_or_default())
                .chain(dmarc)
                .collect::<Vec<_>>()
                .join("\r\n")
        } else {
            format!("{prefix} none")
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    auth_serv_id: String,
}

pub use auth::*;

/// Implementation of the [`RFC "Message Header Field for Indicating Message Authentication Status"`](https://datatracker.ietf.org/doc/html/rfc8601)
#[rhai::plugin::export_module]
mod auth {

    /// Return a new `Authentication-Results` header.
    ///
    /// This method is useful if you want to inspect and add the header to the message yourself.
    /// If you want to create and add the header immediately, use [add_header](http://dev.vsmtp.rs/docs/global/auth#fn-add_header).
    ///
    /// [iprev]:    http://dev.vsmtp.rs/docs/global/iprev
    /// [spf]:      http://dev.vsmtp.rs/docs/global/spf
    /// [dkim]:     http://dev.vsmtp.rs/docs/global/dkim
    /// [dmarc]:    http://dev.vsmtp.rs/docs/global/dmarc
    ///
    /// # Example
    ///
    ///```js
    /// fn on_pre_queue(ctx) {
    ///   let header = auth::create_header(ctx, #{
    ///     auth_serv_id: "mydomain.tld" // The domain name of the authentication server
    ///   });
    ///   log("info", header);
    ///   ctx.prepend_header("Authentication-Results", header);
    ///   status::next()
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(pure, return_raw)]
    pub fn create_header(
        ctx: &mut Ctx,
        params: rhai::Dynamic,
    ) -> Result<String, Box<rhai::EvalAltResult>> {
        const AUTH_RES_VERSION: i32 = 1;
        let Params { auth_serv_id } = rhai::serde::from_dynamic(&params)?;
        let mechanisms = ctx.read(|ctx| Into::<AuthMechanism>::into(ctx));

        Ok(AuthMechanism::make_header(
            mechanisms,
            format!("{auth_serv_id} {AUTH_RES_VERSION};"),
        ))
    }

    /// Add the `Authentication-Results` header to the message.
    /// This method use the result of the previous authentication mechanisms.
    /// See [iprev], [spf], [dkim], [dmarc] for more information.
    ///
    /// [iprev]:    http://dev.vsmtp.rs/docs/global/iprev
    /// [spf]:      http://dev.vsmtp.rs/docs/global/spf
    /// [dkim]:     http://dev.vsmtp.rs/docs/global/dkim
    /// [dmarc]:    http://dev.vsmtp.rs/docs/global/dmarc
    ///
    /// # Example
    ///
    ///```js
    /// fn on_pre_queue(ctx) {
    ///   auth::add_header(ctx, #{
    ///     auth_serv_id: "mydomain.tld" // The domain name of the authentication server
    ///   });
    ///   status::next()
    /// }
    /// ```
    /// # rhai-autodocs:index:2
    #[rhai_fn(pure, return_raw)]
    pub fn add_header(
        ctx: &mut Ctx,
        params: rhai::Dynamic,
    ) -> Result<(), Box<rhai::EvalAltResult>> {
        let body = create_header(ctx, params)?;

        ctx.write(|ctx| {
            ctx.mut_mail(|mail| {
                mail.prepend_headers([Header::new("Authentication-Results", body)]);
            })
        })
        .map_err(StateError::into)
    }
}
