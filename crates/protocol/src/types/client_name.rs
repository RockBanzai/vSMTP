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

use crate::Domain;

/// Identity of the client.
#[derive(
    Debug,
    Clone,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    fake::Dummy,
)]
#[serde(untagged)]
pub enum ClientName {
    /// FQDN of the client.
    Domain(Domain),
    /// IP address of the client.
    Ip4(std::net::Ipv4Addr),
    /// IP address of the client.
    Ip6(std::net::Ipv6Addr),
}

impl std::fmt::Display for ClientName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Domain(domain) => write!(f, "{domain}"),
            Self::Ip4(ip) => write!(f, "{ip}"),
            Self::Ip6(ip) => write!(f, "{ip}"),
        }
    }
}
