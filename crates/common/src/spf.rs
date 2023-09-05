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

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
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
    SoftFail,
    Neutral,
    None,
    TempError,
    PermError,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct SpfResult {
    pub value: Value,
    /// The domain that was queried for.
    /// Wrapped in an option to handle the case where EHLO/HELO is an ip4/ip6 and no domain can be verified.
    pub domain: Option<String>,
}
