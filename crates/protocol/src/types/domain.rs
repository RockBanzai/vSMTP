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

use tokio_rustls::rustls;

/// A domain name.
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
pub struct Domain(#[dummy(faker = "DomainFaker")] hickory_proto::rr::Name);

impl Domain {
    #[must_use]
    pub fn root() -> Self {
        Self(hickory_proto::rr::Name::root())
    }

    pub fn from_utf8(s: &str) -> Result<Self, hickory_proto::error::ProtoError> {
        hickory_proto::rr::Name::from_utf8(s).map(Self)
    }

    #[must_use]
    pub fn zone_of(&self, other: &Self) -> bool {
        self.0.zone_of(&other.0)
    }
}

impl From<Domain> for hickory_proto::rr::Name {
    #[inline]
    fn from(val: Domain) -> Self {
        val.0
    }
}

impl From<hickory_proto::rr::Name> for Domain {
    #[inline]
    fn from(val: hickory_proto::rr::Name) -> Self {
        Self(val)
    }
}

impl TryFrom<Domain> for rustls::ServerName {
    type Error = rustls::client::InvalidDnsNameError;

    fn try_from(value: Domain) -> Result<Self, Self::Error> {
        Self::try_from(value.0.to_string().as_str())
    }
}

impl std::str::FromStr for Domain {
    type Err = hickory_proto::error::ProtoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

impl std::fmt::Display for Domain {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.to_string().fmt(f)
    }
}

pub struct DomainFaker;
impl fake::Dummy<DomainFaker> for hickory_proto::rr::Name {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &DomainFaker, rng: &mut R) -> Self {
        let domain: String =
            fake::Fake::fake_with_rng(&fake::faker::internet::fr_fr::FreeEmailProvider(), rng);
        domain.parse().unwrap()
    }
}

/*

/// An iterator over the domain name.
///
/// # Example
///
/// ```
/// let domain = "www.john.doe.example.com".parse::<vsmtp_common::Domain>().unwrap();
///
/// let domain_str = domain.to_string();
/// let mut domain_part = vsmtp_common::domain_iter(&domain_str);
/// assert_eq!(domain_part.next().unwrap(), "www.john.doe.example.com");
/// assert_eq!(domain_part.next().unwrap(), "john.doe.example.com");
/// assert_eq!(domain_part.next().unwrap(), "doe.example.com");
/// assert_eq!(domain_part.next().unwrap(), "example.com");
/// assert_eq!(domain_part.next().unwrap(), "com");
/// assert_eq!(domain_part.next(), None);
/// ```
#[must_use]
#[inline]
#[allow(clippy::module_name_repetitions)]
pub const fn domain_iter(domain: &str) -> IterDomain<'_> {
    IterDomain::iter(domain)
}

#[allow(clippy::module_name_repetitions)]
pub struct IterDomain<'item>(Option<&'item str>);

impl<'item> IterDomain<'item> {
    /// Create an iterator over the given domain.
    #[must_use]
    pub const fn iter(domain: &'item str) -> Self {
        Self(Some(domain))
    }
}

impl<'item> Iterator for IterDomain<'item> {
    type Item = &'item str;

    fn next(&mut self) -> Option<Self::Item> {
        let out = self.0;
        self.0 = self.0.and_then(|s| s.split_once('.')).map(|(_, rest)| rest);
        out
    }
}

*/
