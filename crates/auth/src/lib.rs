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

//! vSMTP Authentication library
//!
//! SPF / DKIM / DMARC

#![cfg_attr(docsrs, feature(doc_cfg))]
//
#![doc(html_no_source)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
//
#![warn(rust_2018_idioms)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
// #![warn(clippy::restriction)]
//

/// The implementation follow the RFC 7208
///
/// ```txt
/// Email on the Internet can be forged in a number of ways.  In
/// particular, existing protocols place no restriction on what a sending
/// host can use as the "MAIL FROM" of a message or the domain given on
/// the SMTP HELO/EHLO commands.  This document describes version 1 of
/// the Sender Policy Framework (SPF) protocol, whereby ADministrative
/// Management Domains (ADMDs) can explicitly authorize the hosts that
/// are allowed to use their domain names, and a receiving host can check
/// such authorization.
/// ```
pub mod spf;

pub use viaspf;

/// The implementation follow the RFC 6376 & 8301 & 8463
///
/// ```txt
/// DomainKeys Identified Mail (DKIM) permits a person, role, or
/// organization that owns the signing domain to claim some
/// responsibility for a message by associating the domain with the
/// message.  This can be an author's organization, an operational relay,
/// or one of their agents.  DKIM separates the question of the identity
/// of the Signer of the message from the purported author of the
/// message.  Assertion of responsibility is validated through a
/// cryptographic signature and by querying the Signer's domain directly
/// to retrieve the appropriate public key.  Message transit from author
/// to recipient is through relays that typically make no substantive
/// change to the message content and thus preserve the DKIM signature.
/// ```
pub mod dkim;

/// The implementation follow the RFC 7489
///
/// ```txt
/// Domain-based Message Authentication, Reporting, and Conformance
/// (DMARC) is a scalable mechanism by which a mail-originating
/// organization can express domain-level policies and preferences for
/// message validation, disposition, and reporting, that a mail-receiving
/// organization can use to improve mail handling.
/// ```
pub mod dmarc;

///
#[must_use]
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    ///
    #[error("missing required field: `{field}`")]
    MissingRequiredField {
        ///
        field: String,
    },
    ///
    #[error("syntax error: `{reason}`")]
    SyntaxError {
        ///
        reason: String,
    },
    ///
    #[error("invalid argument: `{reason}`")]
    InvalidArgument {
        ///
        reason: String,
    },
}

// FIXME: remove me (only used for strum::EnumIter)
impl Default for ParseError {
    fn default() -> Self {
        Self::InvalidArgument {
            reason: "`default` invoked".to_string(),
        }
    }
}

/// Return the root of a domain
///
/// # Errors
///
/// * could not parse the `domain`
/// * could not retrieve the root of the domain
fn get_root_domain(domain: &str) -> Result<Option<String>, addr::error::Error<'_>> {
    Ok(addr::parse_domain_name(domain)?.root().map(str::to_string))
}
