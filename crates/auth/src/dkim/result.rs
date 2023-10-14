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

use super::Signature;

#[serde_with::serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct DkimVerificationResult {
    pub value: Value,
    /// NOTE: wrapped in an Option if the query/parsing failed
    pub signature: Option<Signature>,
}

/// <https://datatracker.ietf.org/doc/html/rfc8601#section-2.7.1>
#[derive(
    Debug,
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
    None,
    Pass,
    Fail,
    Policy,
    Neutral,
    PermFail,
    TempFail,
}
