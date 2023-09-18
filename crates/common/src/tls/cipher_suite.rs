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

use vsmtp_protocol::rustls;

/// Wrapper around [`rustls::CipherSuite`] to implement [`serde::Deserialize`] and [`serde::Serialize`]
#[derive(
    Debug, Clone, PartialEq, Eq, serde_with::DeserializeFromStr, serde_with::SerializeDisplay,
)]
pub struct CipherSuite(pub rustls::CipherSuite);

impl std::str::FromStr for CipherSuite {
    type Err = String;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TLS_AES_256_GCM_SHA384" => Ok(Self(rustls::CipherSuite::TLS13_AES_256_GCM_SHA384)),
            "TLS_AES_128_GCM_SHA256" => Ok(Self(rustls::CipherSuite::TLS13_AES_128_GCM_SHA256)),
            "TLS_CHACHA20_POLY1305_SHA256" => {
                Ok(Self(rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256))
            }
            "ECDHE_ECDSA_WITH_AES_256_GCM_SHA384" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
            )),
            "ECDHE_ECDSA_WITH_AES_128_GCM_SHA256" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
            )),
            "ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
            )),
            "ECDHE_RSA_WITH_AES_256_GCM_SHA384" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
            )),
            "ECDHE_RSA_WITH_AES_128_GCM_SHA256" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
            )),
            "ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
            )),
            _ => Err(format!("not a valid cipher suite: '{s}'")),
        }
    }
}

impl std::fmt::Display for CipherSuite {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[allow(clippy::wildcard_enum_match_arm)]
        f.write_str(match self.0 {
            rustls::CipherSuite::TLS13_AES_256_GCM_SHA384 => "TLS_AES_256_GCM_SHA384",
            rustls::CipherSuite::TLS13_AES_128_GCM_SHA256 => "TLS_AES_128_GCM_SHA256",
            rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256 => "TLS_CHACHA20_POLY1305_SHA256",
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384 => {
                "ECDHE_ECDSA_WITH_AES_256_GCM_SHA384"
            }
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256 => {
                "ECDHE_ECDSA_WITH_AES_128_GCM_SHA256"
            }
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256 => {
                "ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256"
            }
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384 => {
                "ECDHE_RSA_WITH_AES_256_GCM_SHA384"
            }
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256 => {
                "ECDHE_RSA_WITH_AES_128_GCM_SHA256"
            }
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256 => {
                "ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256"
            }
            _ => "unsupported",
        })
    }
}
