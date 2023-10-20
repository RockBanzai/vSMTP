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
use crate::block_on;
use hickory_resolver::{error::ResolveErrorKind, proto::xfer::retry_dns_handle::RetryableError};
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult,
    TypeId,
};
use vsmtp_common::{
    dns_resolver::DnsResolver,
    hickory_resolver::{self, proto::op::ResponseCode},
};

pub use iprev::*;

// TODO: add a record count lookup max (to avoid DoS on DNS server)
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct IpRevParams {
    ip: std::net::IpAddr,
    #[serde(deserialize_with = "super::deserialize_dns_resolver")]
    dns_resolver: std::sync::Arc<DnsResolver>,
}

#[rhai::plugin::export_module]
mod iprev {
    use vsmtp_auth::iprev::Value;

    /// # rhai-autodocs:index:1
    #[tracing::instrument(skip(params), level = "debug", fields(ip), ret)]
    pub fn check(params: rhai::Dynamic) -> IpRevResult {
        let IpRevParams { ip, dns_resolver } =
            rhai::serde::from_dynamic::<IpRevParams>(&params).unwrap();

        let reverse_lookup = match block_on(dns_resolver.resolver.reverse_lookup(ip)) {
            Ok(reverse_lookup) => reverse_lookup,
            Err(error)
                if error.should_retry()
                    || matches!(
                        error.kind(),
                        ResolveErrorKind::NoConnections
                            | ResolveErrorKind::NoRecordsFound {
                                response_code: ResponseCode::ServFail,
                                ..
                            }
                    ) =>
            {
                tracing::debug!(?error, "DNS error");
                return IpRevResult {
                    value: Value::TempError,
                    ip,
                    fqdn: None,
                };
            }
            Err(error) => {
                tracing::debug!(?error, "DNS error");
                return IpRevResult {
                    value: Value::PermError,
                    ip,
                    fqdn: None,
                };
            }
        };

        for record in reverse_lookup {
            let Ok(ips) = block_on(dns_resolver.resolver.lookup_ip(record.0.clone())) else {
                continue;
            };
            if ips.iter().any(|ip_discovered| ip_discovered == ip) {
                tracing::debug!("Iprev checked");
                return IpRevResult {
                    value: Value::Pass,
                    ip,
                    fqdn: Some(record.0),
                };
            }
        }

        IpRevResult {
            value: Value::Fail,
            ip,
            fqdn: None,
        }
    }

    /// # rhai-autodocs:index:2
    #[rhai_fn(global, pure)]
    pub fn store(ctx: &mut Ctx, iprev: IpRevResult) {
        ctx.write(|ctx| ctx.mut_connect().iprev = Some(iprev));
    }

    /// Result of a IpRev verification run with `iprev::check`.
    ///
    /// # rhai-autodocs:index:3
    type IpRevResult = vsmtp_auth::iprev::IpRevResult;

    /// Get the value of an iprev result as a string.
    ///
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, get = "value", pure)]
    pub fn get_value(res: &mut IpRevResult) -> String {
        res.value.to_string()
    }

    /// Transform a iprev result value to a debug string.
    ///
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, pure)]
    pub fn to_debug(res: &mut IpRevResult) -> String {
        format!("{res:?}")
    }
}
