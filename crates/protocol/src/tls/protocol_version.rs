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

/// Wrapper around [`rustls::ProtocolVersion`] to implement [`serde::Deserialize`] and [`serde::Serialize`]
#[derive(
    Debug, Clone, PartialEq, Eq, serde_with::DeserializeFromStr, serde_with::SerializeDisplay,
)]
pub struct ProtocolVersion(pub rustls::ProtocolVersion);

#[derive(Debug, thiserror::Error)]
#[error("not a valid protocol version")]
pub struct ProtocolVersionFromStrError {
    s: String,
}

impl std::str::FromStr for ProtocolVersion {
    type Err = ProtocolVersionFromStrError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TLSv1_2" | "TLSv1.2" | "0x0303" => Ok(Self(rustls::ProtocolVersion::TLSv1_2)),
            "TLSv1_3" | "TLSv1.3" | "0x0304" => Ok(Self(rustls::ProtocolVersion::TLSv1_3)),
            _ => Err(ProtocolVersionFromStrError { s: s.to_string() }),
        }
    }
}

impl std::fmt::Display for ProtocolVersion {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.as_str().ok_or(std::fmt::Error)?.fmt(f)
    }
}
