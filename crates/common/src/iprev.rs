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

use crate::faker::OptionNameFaker;
use vsmtp_protocol::Domain;

/// <https://datatracker.ietf.org/doc/html/rfc8601#section-2.7.3>
// NOTE: should we keep the fqdn in the state?
#[derive(
    Debug,
    Copy,
    Clone,
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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct IpRevResult {
    pub value: Value,
    pub ip: std::net::IpAddr,
    #[dummy(faker = "OptionNameFaker")]
    pub fqdn: Option<Domain>,
}
