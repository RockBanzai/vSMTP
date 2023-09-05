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

use crate::api::State;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult,
    TypeId,
};
use std::sync::Arc;
use vsmtp_common::{dkim, dmarc, iprev, spf, stateful_ctx_received::StatefulCtxReceived};

struct AuthMechanism {
    iprev: Option<iprev::IpRevResult>,
    spf_helo: Option<Arc<spf::SpfResult>>,
    spf_mail_from: Option<Arc<spf::SpfResult>>,
    dkim: Option<Arc<Vec<dkim::DkimVerificationResult>>>,
    dmarc: Option<Arc<dmarc::Dmarc>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    auth_serv_id: String,
}

pub use auth::*;

/// <https://datatracker.ietf.org/doc/html/rfc8601>
#[rhai::plugin::export_module]
mod auth {
    use vsmtp_common::stateful_ctx_received::StateError;
    use vsmtp_mail_parser::mail::headers::Header;

    // TODO: docs
    /// # rhai-autodocs:index:1
    #[doc(hidden)]
    #[rhai_fn(pure, return_raw)]
    pub fn create_header(
        ctx: &mut State<StatefulCtxReceived>,
        params: rhai::Dynamic,
    ) -> Result<String, Box<rhai::EvalAltResult>> {
        const AUTH_RES_VERSION: i32 = 1;
        let Params { auth_serv_id } = rhai::serde::from_dynamic(&params)?;

        let AuthMechanism {
            iprev,
            spf_helo,
            spf_mail_from,
            dkim,
            dmarc,
        } = ctx.read(|ctx| {
            let iprev = ctx.get_connect().iprev.clone();
            let spf_helo = ctx
                .get_helo()
                .ok()
                .and_then(|helo| helo.spf_helo_identity.clone());
            let spf_mail_from = ctx
                .get_mail_from()
                .ok()
                .and_then(|mail_from| mail_from.spf_mail_from_identity.clone());
            let dkim = ctx
                .get_complete()
                .ok()
                .and_then(|complete| complete.dkim.clone());
            let dmarc = ctx
                .get_complete()
                .ok()
                .and_then(|complete| complete.dmarc.clone());

            AuthMechanism {
                iprev,
                spf_helo,
                spf_mail_from,
                dkim,
                dmarc,
            }
        });

        let out = format!("{auth_serv_id} {AUTH_RES_VERSION};");
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
                dkim.iter()
                    .map(|dkim| {
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

            Ok(std::iter::once(out)
                .chain(iprev)
                .chain(spf_helo)
                .chain(spf_mail_from)
                .chain(dkim.map(Vec::into_iter).unwrap_or_default())
                .chain(dmarc)
                .collect::<Vec<_>>()
                .join("\r\n"))
        } else {
            Ok(format!("{out} none"))
        }
    }

    // TODO: docs
    /// # rhai-autodocs:index:2
    #[doc(hidden)]
    #[rhai_fn(pure, return_raw)]
    pub fn add_header(
        ctx: &mut State<StatefulCtxReceived>,
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
