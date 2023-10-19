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

use super::Record;
use vsmtp_protocol::Domain;

#[derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Copy,
    strum::EnumString,
    strum::Display,
    serde_with::SerializeDisplay,
    serde_with::DeserializeFromStr,
    fake::Dummy,
)]
#[strum(serialize_all = "lowercase")]
pub enum Value {
    Pass,
    Fail,
    TempError,
    PermError,
    None,
}

#[serde_with::serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct Result {
    pub value: Value,
    /// Domain under which the DMARC record was found, from the RFC5322.From domain and
    /// if no record was found from the [Organizational Domain].
    ///
    /// [Organizational Domain]: https://datatracker.ietf.org/doc/html/rfc7489#section-3.2.
    #[dummy(faker = "crate::FreeEmailProvider")]
    pub domain: Domain,
    #[dummy(faker = "crate::FreeEmailProvider")]
    pub rfc5322_from_domain: Domain,
    // NOTE: wrapped in an Option if the query failed
    pub record: Option<Record>,
}
