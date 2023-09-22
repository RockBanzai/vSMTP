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
    Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult, TypeId,
};

pub use net::*;

/// Predefined network ip ranges.
#[rhai::plugin::export_module]
mod net {
    pub type RangeIPv4 = iprange::IpRange<ipnet::Ipv4Net>;

    /// Return an ip range over "192.168.0.0/16".
    ///
    /// # Example
    ///
    /// ```ignore
    /// #{
    ///     rcpt: [
    ///         rule "anti relay" || { if ctx::client_ip() in net::range_192() { state::next() } else { state::deny() } }
    ///     ]
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[must_use]
    #[rhai_fn(name = "range_192")]
    pub fn range_192() -> RangeIPv4 {
        new_rg4("192.168.0.0/16").expect("valid range")
    }

    /// Return an ip range over "172.16.0.0/12".
    ///
    /// # Example
    ///
    /// ```ignore
    /// #{
    ///     rcpt: [
    ///         rule "anti relay" || { if ctx::client_ip() in net::range_172() { state::next() } else { state::deny() } }
    ///     ]
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[must_use]
    #[rhai_fn(name = "range_172")]
    pub fn range_172() -> RangeIPv4 {
        new_rg4("172.16.0.0/12").expect("valid range")
    }

    /// Return an ip range over "10.0.0.0/8".
    ///
    /// # Example
    ///
    /// ```ignore
    /// #{
    ///     rcpt: [
    ///         rule "anti relay" || { if ctx::client_ip() in net::range_10() { state::next() } else { state::deny() } }
    ///     ]
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:3
    #[must_use]
    #[rhai_fn(name = "range_10")]
    pub fn range_10() -> RangeIPv4 {
        new_rg4("10.0.0.0/8").expect("valid range")
    }

    /// Return a list of non routable networks (`net_192`, `net_172`, and `net_10`).
    ///
    /// # rhai-autodocs:index:4
    #[must_use]
    #[rhai_fn(name = "non_routable")]
    pub fn non_routable() -> rhai::Array {
        rhai::Array::from_iter([
            rhai::Dynamic::from(range_192()),
            rhai::Dynamic::from(range_172()),
            rhai::Dynamic::from(range_10()),
        ])
    }
}

fn new_rg4(range: impl AsRef<str>) -> Result<iprange::IpRange<ipnet::Ipv4Net>> {
    range
        .as_ref()
        .parse::<ipnet::Ipv4Net>()
        .map(|range| std::iter::once(range).collect::<iprange::IpRange<ipnet::Ipv4Net>>())
        .map_err(|error| {
            format!("failed to parse ip4 range `{}`: {}", range.as_ref(), error).into()
        })
}
