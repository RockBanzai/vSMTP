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
use crate::block_on;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use trust_dns_resolver::{error::ResolveErrorKind, proto::xfer::retry_dns_handle::RetryableError};
use vsmtp_common::{
    dns_resolver::DnsResolver,
    iprev::IpRevResult,
    iprev::Value,
    stateful_ctx_received::StatefulCtxReceived,
    trust_dns_resolver::{self, proto::op::ResponseCode},
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

    /// # rhai-autodocs:index:1
    #[rhai_fn(global, pure)]
    pub fn to_debug(res: &mut IpRevResult) -> String {
        format!("{res:?}")
    }

    /// # rhai-autodocs:index:2
    #[rhai_fn(global, name = "==", pure)]
    pub fn equal_to_str(lhs: &mut Value, rhs: &str) -> bool {
        matches!(
            (lhs, rhs),
            (Value::Pass, "pass")
                | (Value::Fail, "fail")
                | (Value::TempError, "temperror")
                | (Value::PermError, "permerror")
        )
    }

    /// # rhai-autodocs:index:3
    #[rhai_fn(global, name = "!=", pure)]
    pub fn not_equal_to_str(lhs: &mut Value, rhs: &str) -> bool {
        !equal_to_str(lhs, rhs)
    }

    /// # rhai-autodocs:index:4
    #[rhai_fn(global, get = "value", pure)]
    pub fn get_value(res: &mut IpRevResult) -> Value {
        res.value
    }

    /// # rhai-autodocs:index:5
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

    /// # rhai-autodocs:index:6
    #[rhai_fn(global, pure)]
    pub fn store(ctx: &mut State<StatefulCtxReceived>, iprev: IpRevResult) {
        ctx.write(|ctx| ctx.mut_connect().iprev = Some(iprev));
    }
}
