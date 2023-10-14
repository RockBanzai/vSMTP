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
pub struct Dmarc {
    pub value: Value,
    #[dummy(faker = "crate::FreeEmailProvider")]
    pub domain: Domain,
    // NOTE: wrapped in an Option if the query failed
    pub record: Option<Record>,
}
