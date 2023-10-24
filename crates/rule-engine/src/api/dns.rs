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
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_common::hickory_resolver::proto::rr::RecordType;

pub use dns::*;

/// Functions used to query the DNS.
#[rhai::plugin::export_module]
mod dns {
    /// DNS resolver instance created using `dns::resolver`.
    /// Can be used to perform lookups and reverse lookups, see `dns::lookup` and `dns::rlookup`.
    ///
    /// # rhai-autodocs:index:1
    pub type DnsResolver = std::sync::Arc<vsmtp_common::dns_resolver::DnsResolver>;

    /// Create an instance of a DNS resolver.
    ///
    /// # Args
    ///
    /// * `config` - The configuration for the connection to the DNS server, see [`ResolverConfig`].
    /// * `option` - The resolver options, see [`ResolverOpts`].
    ///
    /// # Example
    ///
    /// ```js
    ///  // using a build-in dns server config among:
    ///  // * `google`      / `google_tls`
    ///  // * `cloudflare`  / `cloudflare_tls`
    ///  // * `quad9`       / `quad9_tls`
    /// const google_dns = dns::resolver(#{
    ///    config: "google_tls",
    ///    option: #{
    ///      validate: true,                // use DNSSEC to validate the request,
    ///      ip_strategy: "Ipv6thenIpv4",   // The ip_strategy for the Resolver to use when lookup Ipv4 or Ipv6 addresses
    ///      edns0: true,                   // Enable edns, for larger records
    ///      // and more...
    ///    }
    /// });
    /// ```
    ///
    /// or, with a custom config:
    ///
    /// ```js
    /// const custom_dns = dns::resolver(#{
    ///   config: #{
    ///     nameservers: [
    ///       "socket_addr": "127.0.0.1:853",
    ///       "protocol": "quic",
    ///       // and more...
    ///     ]
    ///   },
    /// });
    /// ```
    ///
    /// [`ResolverConfig`]: https://docs.rs/hickory-resolver/latest/hickory_resolver/config/struct.ResolverConfig.html
    /// [`ResolverOpts`]: https://docs.rs/hickory-resolver/latest/hickory_resolver/config/struct.ResolverOpts.html
    /// # rhai-autodocs:index:2
    #[rhai_fn(return_raw)]
    pub fn resolver(params: &mut rhai::Dynamic) -> Result<DnsResolver> {
        match rhai::serde::from_dynamic(params) {
            Ok(resolver) => Ok(std::sync::Arc::new(resolver)),
            Err(e) => Err(serde::de::Error::custom(format!(
                "failed to parse dns resolver: {e}"
            ))),
        }
    }

    /// Performs a dual-stack DNS lookup for the given hostname.
    ///
    /// # Args
    ///
    /// * `host`   - A valid hostname to search.
    /// * `record` - A valid record type to search as a string. (optional, default: "A"/"AAAA")
    ///              Can be "A", "AAAA", "MX", "TXT", "TLSA", etc.
    ///
    /// # Return
    ///
    /// * `array` - an array of IPs. The array is empty if no IPs were found for the host.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Errors
    ///
    /// * Root resolver was not found.
    /// * Lookup failed.
    ///
    /// # Examples
    ///
    /// ```js
    /// const google_dns = dns::resolver(#{
    ///    config: "google_tls",
    /// });
    ///
    /// // Logging all ip attached to the `google.com` domain.
    /// // Calling `lookup` this way will search for A and AAAA records.
    /// for ip in google_dns.lookup("google.com") {
    ///     log("my_topic", "debug", ip);
    /// }
    ///
    /// // Logging all mail exchangers attached to the `google.com` domain
    /// // asking the resolver for MX records.
    /// for mx in google_dns.lookup("google.com", "MX") {
    ///     log("my_topic", "debug", mx);
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:3
    #[rhai_fn(global, name = "lookup", return_raw, pure)]
    pub fn lookup(dns_resolver: &mut DnsResolver, host: &str) -> Result<rhai::Array> {
        Ok(crate::block_on(dns_resolver.resolver.lookup_ip(host))
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?
            .into_iter()
            .map(|record| rhai::Dynamic::from(record.to_string()))
            .collect::<rhai::Array>())
    }

    #[doc(hidden)]
    #[rhai_fn(global, name = "lookup", return_raw, pure)]
    pub fn lookup_record(
        dns_resolver: &mut DnsResolver,
        host: &str,
        record: &str,
    ) -> Result<rhai::Array> {
        let record = <RecordType as std::str::FromStr>::from_str(record)
            .map_err::<Box<rhai::EvalAltResult>, _>(|_| {
                format!("Invalid record type {record}").into()
            })?;

        Ok(crate::block_on(dns_resolver.resolver.lookup(host, record))
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?
            .into_iter()
            .map(|record| rhai::Dynamic::from(record.to_string()))
            .collect::<rhai::Array>())
    }

    /// Performs a reverse lookup for the given IP.
    ///
    /// # Args
    ///
    /// * `ip` - The IP to query.
    ///
    /// # Return
    ///
    /// * `array` - an array of FQDNs. The array is empty if nothing was found.
    ///
    /// # SMTP stages
    ///
    /// All of them.
    ///
    /// # Errors
    ///
    /// * Failed to convert the `ip` parameter from a string into an IP.
    /// * Reverse lookup failed.
    ///
    /// # Examples
    ///
    /// ```js
    /// const google_dns = dns::resolver(#{
    ///    config: "google_tls",
    /// });
    ///
    /// // Logging all domain attached to the `x.x.x.x` ip address.
    /// for domain in google_dns.rlookup("x.x.x.x") {
    ///     log("my_topic", "debug", domain);
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, name = "rlookup", return_raw)]
    pub fn rlookup(dns_resolver: &mut DnsResolver, ip: &str) -> Result<rhai::Array> {
        let ip = <std::net::IpAddr as std::str::FromStr>::from_str(ip)
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?;

        Ok(crate::block_on(dns_resolver.resolver.reverse_lookup(ip))
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?
            .into_iter()
            .map(|record| rhai::Dynamic::from(record.to_string()))
            .collect::<rhai::Array>())
    }
}
