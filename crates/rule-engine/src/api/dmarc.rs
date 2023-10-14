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
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_auth::dmarc as backend;
use vsmtp_common::dns_resolver::DnsResolver;
use vsmtp_mail_parser::{mail::headers::Header, Mail};

pub use rhai_dmarc::*;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    #[serde(deserialize_with = "super::deserialize_dns_resolver")]
    dns_resolver: std::sync::Arc<DnsResolver>,
}

fn get_rfc5322_from_domain(msg: &Mail) -> Result<String, String> {
    let Header { body, .. } = msg
        .get_rfc5322_from()
        .ok_or("Header field `From` is not RFC 5322 valid: missing `From` header field")?;

    let address_pos = body
        .find('<')
        .and_then(|begin| body.find('>').map(|end| (begin, end)));

    let body = match address_pos {
        Some((start, end)) => &body[start + 1..end],
        None => body.strip_suffix("\r\n").unwrap_or(body),
    };

    let rfc5322_from_domain = body
        .find('@')
        .map(|at| &body[at + 1..])
        .ok_or("Header field `From` is not RFC 5322 valid")?;

    Ok(rfc5322_from_domain.to_string())
}

// TODO: enhance RFC compliance https://datatracker.ietf.org/doc/html/rfc7489#section-6.6.3
async fn get_dmarc_record(
    dns_resolver: std::sync::Arc<DnsResolver>,
    rfc5322_from_domain: &str,
) -> Result<backend::Record, backend::Value> {
    match dns_resolver
        .resolver
        .txt_lookup(format!("_dmarc.{rfc5322_from_domain}"))
        .await
    {
        Ok(record) if record.iter().count() != 1 => {
            tracing::debug!("No DMARC record found");
            Err(backend::Value::None)
        }
        Ok(record) => {
            let record = record.into_iter().next().expect("count == 1");
            match <backend::Record as std::str::FromStr>::from_str(&record.to_string()) {
                Ok(dmarc_record) => Ok(dmarc_record),
                Err(e) => {
                    tracing::debug!(?e, "Invalid DMARC record");
                    Err(backend::Value::None)
                }
            }
        }
        Err(e) => {
            tracing::debug!(?e, "DNS error");
            Err(backend::Value::TempError)
        }
    }
}

type DmarcResult = rhai::Shared<backend::Dmarc>;
type DmarcValue = backend::Value;

/// Domain-based message authentication, reporting and conformance implementation
/// specified by RFC 7489. (<https://www.rfc-editor.org/rfc/rfc7489>)
#[rhai::plugin::export_module]
mod rhai_dmarc {

    /// Execute a DMARC policy check.
    ///
    /// # Parameters
    ///
    /// a map composed of the following parameters:
    /// - `dns_resolver`: The DNS resolver to use when performing DMARC record lookup. (see the `dns` module)
    ///
    /// # Examples
    ///
    /// Here is a standard DMARC policy handling that you can setup using scripting.
    ///
    /// ```js
    /// fn on_pre_queue(ctx) {
    ///     let dmarc_result = dmarc::check(ctx, #{ dns_resolver: global::dns_resolver });
    ///     ctx.store(dmarc_result);
    ///
    ///     if dmarc_result.value == "pass" {
    ///         status::next()
    ///     } else {
    ///         // Decide what to do following the policy.
    ///         let policy = dmarc_result.policy;
    ///         switch policy {
    ///             "none" => {
    ///                 log("my_topic", "warn", "the message failed the DMARC check but DMARC policy is none, so ignoring");
    ///                 status::next()
    ///             }
    ///             "quarantine" => status::quarantine("dmarc"),
    ///             "reject" => status::deny(`550 5.7.25 DMARC policy violation`),
    ///             _ => throw "unknown DMARC policy"
    ///         }
    ///     }
    /// }
    /// ```
    /// # rhai-autodocs:index:1
    #[rhai_fn(pure, return_raw)]
    pub fn check(
        ctx: &mut Ctx,
        params: rhai::Dynamic,
    ) -> Result<DmarcResult, Box<rhai::EvalAltResult>> {
        let Params { dns_resolver } = rhai::serde::from_dynamic(&params)?;

        let (rfc5322_from_domain, spf, dkim) =
            ctx.read(|ctx| match ctx.get_mail(get_rfc5322_from_domain) {
                Err(e) => Err(e.to_string()),
                Ok(Err(e)) => Err(e),
                Ok(Ok(rfc5322_from_domain)) => Ok((
                    rfc5322_from_domain,
                    ctx.get_mail_from()
                        .map_err(|e| e.to_string())?
                        .spf_mail_from_identity
                        .clone()
                        .ok_or("SPF on MAIL FROM identity must be called first")?,
                    ctx.get_complete()
                        .map_err(|e| e.to_string())?
                        .dkim
                        .clone()
                        .ok_or("DKIM must be called first")?,
                )),
            })?;

        let record = match crate::block_on(get_dmarc_record(dns_resolver, &rfc5322_from_domain)) {
            Ok(record) => record,
            Err(value) => {
                return Ok(backend::Dmarc {
                    value,
                    domain: rfc5322_from_domain.parse().unwrap(),
                    record: None,
                }
                .into())
            }
        };

        if spf.value == vsmtp_auth::spf::Value::Pass
            && spf
                .domain
                .as_deref()
                .is_some_and(|spf_domain| record.spf_is_aligned(&rfc5322_from_domain, spf_domain))
        {
            return Ok(backend::Dmarc {
                value: backend::Value::Pass,
                domain: rfc5322_from_domain.parse().unwrap(),
                record: Some(record),
            }
            .into());
        }

        for i in &*dkim {
            if i.value == vsmtp_auth::dkim::Value::Pass
                && i.signature.as_ref().is_some_and(|signature| {
                    record.dkim_is_aligned(&rfc5322_from_domain, &signature.sdid)
                })
            {
                tracing::debug!("Dmarc signature checked");
                return Ok(backend::Dmarc {
                    value: backend::Value::Pass,
                    domain: rfc5322_from_domain.parse().unwrap(),
                    record: Some(record),
                }
                .into());
            }
        }

        Ok(backend::Dmarc {
            value: backend::Value::Fail,
            domain: rfc5322_from_domain.parse().unwrap(),
            record: Some(record),
        }
        .into())
    }

    /// Cache DMARC result from the `dmarc::check` function.
    /// # rhai-autodocs:index:2
    #[rhai_fn(global, pure, return_raw)]
    pub fn store(ctx: &mut Ctx, dmarc_result: DmarcResult) -> Result<(), Box<rhai::EvalAltResult>> {
        ctx.write(|ctx| {
            ctx.mut_complete()?.dmarc = Some(dmarc_result);
            Ok(())
        })
    }

    /// Get the value of the dmarc result after calling the `dmarc::check` function.
    /// # rhai-autodocs:index:3
    #[rhai_fn(global, get = "value", pure)]
    pub fn get_value(res: &mut DmarcResult) -> DmarcValue {
        res.value
    }

    // TODO: if the RFC5322's domain is a subdomain of of the Organizational Domain AND, then record's subdomain policy must be used
    /// Get the policy fetched from the DMARC records.
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, get = "policy", pure)]
    pub fn get_policy(res: &mut DmarcResult) -> String {
        res.record
            .as_ref()
            .map_or_else(|| "none".to_string(), backend::Record::get_policy)
    }

    /// Compare dmarc result as equal to a string.
    ///
    /// # Examples
    ///
    /// ```js
    /// let result = dmarc::check(#{ dns_resolver: global::dns_resolver });
    ///
    /// // The `==` operator is used here.
    /// if result == "pass" {
    ///     // ...
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, name = "==", pure)]
    pub fn equal_to_str(lhs: &mut DmarcValue, rhs: &str) -> bool {
        matches!(
            (lhs, rhs),
            (DmarcValue::Pass, "pass")
                | (DmarcValue::Fail, "fail")
                | (DmarcValue::None, "none")
                | (DmarcValue::TempError, "temperror")
                | (DmarcValue::PermError, "permerror")
        )
    }

    /// Compare dmarc result as not equal to a string.
    ///
    /// # Examples
    ///
    /// ```js
    /// let result = dmarc::check(#{ dns_resolver: global::dns_resolver });
    ///
    /// // The `==` operator is used here.
    /// if result != "pass" {
    ///     // ...
    /// }
    /// ```
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, name = "!=", pure)]
    pub fn not_equal_to_str(lhs: &mut DmarcValue, rhs: &str) -> bool {
        !equal_to_str(lhs, rhs)
    }
}
