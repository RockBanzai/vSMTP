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
    Copy,
    Clone,
    PartialEq,
    Eq,
    strum::AsRefStr,
    strum::Display,
    strum::EnumString,
    strum::EnumVariantNames,
    serde_with::DeserializeFromStr,
    serde_with::SerializeDisplay,
    fake::Dummy,
)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Extension {
    StartTls,
    Auth,
    Pipelining,
    #[strum(serialize = "8BITMIME")]
    BitMime8,
    EnhancedStatusCodes,
    #[strum(serialize = "DSN")]
    DeliveryStatusNotification,
    Unknown,
}

#[allow(clippy::string_slice, clippy::indexing_slicing, clippy::expect_used)]
#[must_use]
pub fn from_str(input: &str) -> (Extension, &str) {
    <Extension as strum::VariantNames>::VARIANTS
        .iter()
        .find(|i| input.len() >= i.len() && input[..i.len()].eq_ignore_ascii_case(i))
        .map_or_else(
            || (Extension::Unknown, input),
            |verb| {
                (
                    verb.parse().expect("extension found above"),
                    &input[verb.len()..],
                )
            },
        )
}
