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
use crate::api::Result;
use crate::block_on;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_auth::spf;
use vsmtp_common::{dns_resolver::DnsResolver, hickory_resolver};
use vsmtp_protocol::ClientName;

pub use rhai_spf::*;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    ip: std::net::IpAddr,
    helo: ClientName,
    mail_from: Option<rhai::Dynamic>,
    #[serde(deserialize_with = "super::deserialize_dns_resolver")]
    dns_resolver: std::sync::Arc<DnsResolver>,
}

struct Lookup(std::sync::Arc<DnsResolver>);

fn to_lookup_error(error: hickory_resolver::error::ResolveError) -> viaspf::lookup::LookupError {
    match error.kind() {
        hickory_resolver::error::ResolveErrorKind::NoRecordsFound { .. } => {
            viaspf::lookup::LookupError::NoRecords
        }
        hickory_resolver::error::ResolveErrorKind::Timeout => viaspf::lookup::LookupError::Timeout,
        _ => wrap_error(error),
    }
}

fn wrap_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> viaspf::lookup::LookupError {
    viaspf::lookup::LookupError::Dns(Some(error.into()))
}

fn to_trust_dns_name(
    name: &viaspf::lookup::Name,
) -> viaspf::lookup::LookupResult<hickory_resolver::Name> {
    hickory_resolver::Name::from_ascii(name).map_err(wrap_error)
}

#[async_trait::async_trait]
impl viaspf::lookup::Lookup for Lookup {
    async fn lookup_a<'lookup, 'a>(
        &'lookup self,
        name: &'a viaspf::lookup::Name,
    ) -> viaspf::lookup::LookupResult<Vec<std::net::Ipv4Addr>> {
        Ok(self
            .0
            .resolver
            .ipv4_lookup(to_trust_dns_name(name)?)
            .await
            .map_err(to_lookup_error)?
            .into_iter()
            .map(|i| i.0)
            .collect())
    }

    async fn lookup_aaaa<'lookup, 'a>(
        &'lookup self,
        name: &'a viaspf::lookup::Name,
    ) -> viaspf::lookup::LookupResult<Vec<std::net::Ipv6Addr>> {
        Ok(self
            .0
            .resolver
            .ipv6_lookup(to_trust_dns_name(name)?)
            .await
            .map_err(to_lookup_error)?
            .into_iter()
            .map(|i| i.0)
            .collect())
    }

    async fn lookup_mx<'lookup, 'a>(
        &'lookup self,
        name: &'a viaspf::lookup::Name,
    ) -> viaspf::lookup::LookupResult<Vec<viaspf::lookup::Name>> {
        let mut mxs = self
            .0
            .resolver
            .mx_lookup(to_trust_dns_name(name)?)
            .await
            .map_err(to_lookup_error)?
            .into_iter()
            .collect::<Vec<_>>();
        mxs.sort_by_key(hickory_resolver::proto::rr::rdata::MX::preference);
        mxs.into_iter()
            .map(|mx| viaspf::lookup::Name::new(&mx.exchange().to_ascii()).map_err(wrap_error))
            .collect()
    }

    async fn lookup_txt<'lookup, 'a>(
        &'lookup self,
        name: &'a viaspf::lookup::Name,
    ) -> viaspf::lookup::LookupResult<Vec<String>> {
        Ok(self
            .0
            .resolver
            .txt_lookup(to_trust_dns_name(name)?)
            .await
            .map_err(to_lookup_error)?
            .into_iter()
            .map(|txt| {
                txt.iter()
                    .map(|data| String::from_utf8_lossy(data))
                    .collect()
            })
            .collect())
    }

    async fn lookup_ptr<'lookup>(
        &'lookup self,
        ip: std::net::IpAddr,
    ) -> viaspf::lookup::LookupResult<Vec<viaspf::lookup::Name>> {
        self.0
            .resolver
            .reverse_lookup(ip)
            .await
            .map_err(to_lookup_error)?
            .into_iter()
            .map(|name| viaspf::lookup::Name::new(&name.to_ascii()).map_err(wrap_error))
            .collect()
    }
}

fn to_spf_result(
    viaspf::QueryResult {
        spf_result,
        cause: _,
        trace: _,
    }: viaspf::QueryResult,
    domain: String,
) -> spf::Result {
    spf::Result {
        value: match spf_result {
            viaspf::SpfResult::None => spf::Value::None,
            viaspf::SpfResult::Neutral => spf::Value::Neutral,
            viaspf::SpfResult::Pass => spf::Value::Pass,
            viaspf::SpfResult::Fail(_) => spf::Value::Fail,
            viaspf::SpfResult::Softfail => spf::Value::SoftFail,
            viaspf::SpfResult::Temperror => spf::Value::TempError,
            viaspf::SpfResult::Permerror => spf::Value::PermError,
        },
        domain: Some(domain),
    }
}

/// Implementation of the Sender Policy Framework (SPF), described by RFC 7208. (<https://datatracker.ietf.org/doc/html/rfc7208>)
#[rhai::plugin::export_module]
mod rhai_spf {

    /// Execute SPF (Sender Policy Framework) verification with check if the client is authorized to
    /// send mail on behalf of the sender's domain.
    ///
    /// # Arguments
    ///
    /// * `params` - A map containing the following parameters:
    ///   * `ip`           - The ip of the sender.
    ///   * `helo`         - The HELO/EHLO command sent by the client.
    ///   * `mail_from`    - The sender of the email.
    ///   * `dns_resolver` - The DNS resolver to use for the verification, loaded with the [dns] module.
    ///
    /// [dns]: http://vsmtp.rs/docs/global/dns
    ///
    /// # Example
    ///
    /// ```js
    /// fn on_pre_queue(ctx) {
    ///     let helo_identity = spf::check_host(#{
    ///         ip: ctx.client_ip,
    ///         helo: ctx.helo,
    ///         mail_from: ctx.sender,
    ///         dns_resolver: global::dns_resolver
    ///     });
    ///     if helo_identity != "pass" {
    ///         status::deny(`550 5.7.23 ${ctx.helo} is not allowed to send mail from ${ctx.client_ip}`)
    ///     } else {
    ///         ctx.store("helo", helo_identity);
    ///         status::next()
    ///     }
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(return_raw)] // NOTE: should return a tempfail to handle user's rules issues??
    #[tracing::instrument(skip(params), ret, err)]
    pub fn check_host(params: rhai::Dynamic) -> Result<SpfResult> {
        let Params {
            ip,
            helo,
            mail_from,
            dns_resolver,
        } = rhai::serde::from_dynamic(&params)?;

        let helo = match helo {
            ClientName::Ip4(..) | ClientName::Ip6(..) => {
                return Ok(spf::Result {
                    value: spf::Value::None,
                    domain: None,
                }
                .into());
            }
            ClientName::Domain(helo) => helo,
        };

        // NOTE: if the `mail_from` is set, we check for the MAIL FROM identity.
        if let Some(mail_from) = mail_from {
            // NOTE: if the reverse path is null, message is assumed to be issued by the sender aka helo.
            if mail_from
                .clone()
                .try_cast::<String>()
                .is_some_and(|i| i == "<>")
            {
                let sender = viaspf::Sender::from_domain(&helo.to_string()).unwrap();

                Ok(to_spf_result(
                    block_on(viaspf::evaluate_sender(
                        &Lookup(dns_resolver),
                        &viaspf::Config::builder().build(),
                        ip,
                        &sender,
                        Some(sender.domain()),
                    )),
                    sender.domain().to_string(),
                )
                .into())

                // NOTE: otherwise, we check for the MAIL FROM 's domain.
            } else if let Some(mail_from) = mail_from.try_cast::<vsmtp_common::Mailbox>() {
                let sender = viaspf::Sender::from_address(&mail_from.0.to_string()).unwrap();
                let helo = helo.to_string().parse().unwrap();

                Ok(to_spf_result(
                    block_on(viaspf::evaluate_sender(
                        &Lookup(dns_resolver),
                        &viaspf::Config::builder().build(),
                        ip,
                        &sender,
                        Some(&helo),
                    )),
                    sender.domain().to_string(),
                )
                .into())
            } else {
                return Err("`mail_from` is not a valid mailbox".to_string().into());
            }

        // NOTE: otherwise we check for the HELO identity.
        } else {
            let sender = viaspf::Sender::from_domain(&helo.to_string()).unwrap();

            Ok(to_spf_result(
                block_on(viaspf::evaluate_sender(
                    &Lookup(dns_resolver),
                    &viaspf::Config::builder().build(),
                    ip,
                    &sender,
                    Some(sender.domain()),
                )),
                sender.domain().to_string(),
            )
            .into())
        }
    }

    /// Store the result of a previous `spf::check_host` function execution.
    /// # rhai-autodocs:index:2
    #[rhai_fn(global, pure, return_raw)]
    pub fn store(ctx: &mut Ctx, identity: &str, spf_result: SpfResult) -> Result<()> {
        match identity {
            "helo" => ctx.write(|ctx| {
                ctx.mut_helo().map_err(|e| e.to_string())?.spf_helo_identity = Some(spf_result);
                Ok(())
            }),
            "mail_from" => ctx.write(|ctx| {
                ctx.mut_mail_from()
                    .map_err(|e| e.to_string())?
                    .spf_mail_from_identity = Some(spf_result);
                Ok(())
            }),
            otherwise => Err(format!("unknown identity: {otherwise}").into()),
        }
    }

    /// Result of a SPF verification run with `spf::check_host`.
    ///
    /// # rhai-autodocs:index:3
    pub type SpfResult = rhai::Shared<vsmtp_auth::spf::Result>;

    /// Get the value of the SPF result.
    ///
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, get = "value", pure)]
    pub fn value(v: &mut SpfResult) -> String {
        v.value.to_string()
    }

    /// Transform the spf result into a debug string.
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, pure)]
    pub fn to_debug(v: &mut SpfResult) -> String {
        format!("{v:?}")
    }
}
