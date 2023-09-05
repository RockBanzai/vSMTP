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

/// Type of SMTP connection.
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    strum::Display,
    strum::EnumString,
    serde_with::DeserializeFromStr,
    serde_with::SerializeDisplay,
)]
#[strum(serialize_all = "lowercase")]
#[non_exhaustive]
pub enum ConnectionKind {
    /// Connection coming for relay (MTA on port 25)
    /// see <https://datatracker.ietf.org/doc/html/rfc5321>
    #[default]
    Relay,
    /// Connection coming for submission (MSA on port 587)
    /// see <https://datatracker.ietf.org/doc/html/rfc6409>
    Submission,
    /// Connection coming for submissionS (MSA on port 465)
    /// see <https://datatracker.ietf.org/doc/html/rfc8314>
    Tunneled,
}
