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

/// A domain name.
pub type Domain = trust_dns_proto::rr::Name;

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
