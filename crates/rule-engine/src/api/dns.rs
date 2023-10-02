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

pub use dns::*;

pub type DnsResolver = std::sync::Arc<vsmtp_common::dns_resolver::DnsResolver>;

/// Functions used to query the DNS.
#[rhai::plugin::export_module]
mod dns {
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
    /// [`ResolverConfig`]: https://docs.rs/trust-dns-resolver/latest/trust_dns_resolver/config/struct.ResolverConfig.html
    /// [`ResolverOpts`]: https://docs.rs/trust-dns-resolver/latest/trust_dns_resolver/config/struct.ResolverOpts.html
    /// # rhai-autodocs:index:1
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
    /// * `host` - A valid hostname to search.
    ///
    /// # Return
    ///
    /// * `array` - an array of IPs. The array is empty if no IPs were found for the host.
    ///
    /// # Effective smtp stage
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
    /// # vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///   preq: [
    ///     action "lookup recipients" || {
    ///       let domain = "gmail.com";
    ///       let ips = dns::lookup(domain);
    ///
    ///       print(`ips found for ${domain}`);
    ///       for ip in ips { print(`- ${ip}`); }
    ///     },
    ///   ],
    /// }
    /// # "#)?.build()));
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(name = "lookup", return_raw)]
    pub fn lookup(dns: &mut DnsResolver, host: &str) -> Result<rhai::Array> {
        // NOTE: should lookup & rlookup return an error if no record was found ?

        Ok(crate::block_on(dns.resolver.lookup_ip(host))
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
    /// # Effective smtp stage
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
    /// # let states = vsmtp_test::rhai::run(
    /// # |builder| Ok(builder.add_root_filter_rules(r#"
    /// #{
    ///   connect: [
    ///     rule "rlookup" || {
    ///       state::accept(`250 client ip: ${"127.0.0.1"} -> ${dns::rlookup("127.0.0.1")}`);
    ///     }
    ///   ],
    /// }
    /// # "#)?.build()));
    /// # use vsmtp_common::{status::Status, Reply, ReplyCode::Code};
    /// # assert_eq!(states[&vsmtp_rule_engine::ExecutionStage::Connect].2, Status::Accept(
    /// #  r#"250 client ip: 127.0.0.1 -> ["localhost."]"#.parse().unwrap(),
    /// # ));
    /// ```
    ///
    /// # rhai-autodocs:index:3
    #[rhai_fn(name = "rlookup", return_raw)]
    pub fn rlookup(dns: &mut DnsResolver, ip: &str) -> Result<rhai::Array> {
        let ip = <std::net::IpAddr as std::str::FromStr>::from_str(ip)
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?;

        Ok(crate::block_on(dns.resolver.reverse_lookup(ip))
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?
            .into_iter()
            .map(|record| rhai::Dynamic::from(record.to_string()))
            .collect::<rhai::Array>())
    }
}
