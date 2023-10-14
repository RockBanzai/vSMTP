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
use crate::dkim;

#[derive(Debug, thiserror::Error)]
pub enum TlsPrivateKeyError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("Cannot parse pem data")]
    InvalidPem,
    #[error("{0}")]
    Pkcs1(#[from] rsa::pkcs1::Error),
    #[error("{0}")]
    Pkcs8(#[from] rsa::pkcs8::Error),
    #[error("{0}")]
    Ed25519(#[from] ring_compat::ring::error::KeyRejected),
    #[error("the private key is not in a supported format (pem+rsa+pkcs1, pem+rsa+pkcs8, pem+ed25519+pkcs8)")]
    CannotParse,
}

#[derive(Debug, PartialEq, Eq, serde_with::DeserializeFromStr)]
pub struct TlsPrivateKey {
    source: Box<str>,
    private_key: dkim::PrivateKey,
}

impl std::str::FromStr for TlsPrivateKey {
    type Err = TlsPrivateKeyError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::load_any(s)
    }
}

impl serde::Serialize for TlsPrivateKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.source)
    }
}

impl TlsPrivateKey {
    #[must_use]
    pub const fn private_key(&self) -> &dkim::PrivateKey {
        &self.private_key
    }

    fn load_pem_rsa_pkcs1(
        source: &str,
    ) -> std::result::Result<dkim::PrivateKey, TlsPrivateKeyError> {
        let private_key =
            <rsa::RsaPrivateKey as rsa::pkcs1::DecodeRsaPrivateKey>::from_pkcs1_pem(source)?;
        Ok(dkim::PrivateKey::Rsa(Box::new(private_key)))
    }

    fn load_pem_rsa_pkcs8(
        source: &str,
    ) -> std::result::Result<dkim::PrivateKey, TlsPrivateKeyError> {
        let private_key =
            <rsa::RsaPrivateKey as rsa::pkcs8::DecodePrivateKey>::from_pkcs8_pem(source)?;
        Ok(dkim::PrivateKey::Rsa(Box::new(private_key)))
    }

    fn load_pem_ed_pkcs8(
        source: &str,
    ) -> std::result::Result<dkim::PrivateKey, TlsPrivateKeyError> {
        let (_type_label, data) = pem_rfc7468::decode_vec(source.as_bytes())
            .map_err(|_| TlsPrivateKeyError::CannotParse)?;

        let ed25519 =
            ring_compat::ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&data)?;

        Ok(dkim::PrivateKey::Ed25519(Box::new(ed25519)))
    }

    pub fn load_pem_rsa_pkcs1_file(
        filepath: &str,
    ) -> std::result::Result<Self, TlsPrivateKeyError> {
        let source = std::fs::read_to_string(filepath)?;
        Self::load_pem_rsa_pkcs1(&source).map(|private_key| Self {
            source: source.into(),
            private_key,
        })
    }

    pub fn load_pem_rsa_pkcs8_file(
        filepath: &str,
    ) -> std::result::Result<Self, TlsPrivateKeyError> {
        let source = std::fs::read_to_string(filepath)?;
        Self::load_pem_rsa_pkcs8(&source).map(|private_key| Self {
            source: source.into(),
            private_key,
        })
    }

    pub fn load_pem_ed_pkcs8_file(filepath: &str) -> std::result::Result<Self, TlsPrivateKeyError> {
        let source = std::fs::read_to_string(filepath)?;
        Self::load_pem_ed_pkcs8(&source).map(|private_key| Self {
            source: source.into(),
            private_key,
        })
    }

    fn load_any(source: &str) -> std::result::Result<Self, TlsPrivateKeyError> {
        Self::load_pem_rsa_pkcs8(source)
            .or_else(|_| Self::load_pem_ed_pkcs8(source))
            .or_else(|_| Self::load_pem_rsa_pkcs1(source))
            .map_or_else(
                |_| Err(TlsPrivateKeyError::CannotParse),
                |private_key| {
                    Ok(Self {
                        source: source.into(),
                        private_key,
                    })
                },
            )
    }

    // fn load_any_file(filepath: &str) -> std::result::Result<Self, TlsPrivateKeyError> {
    //     let content = std::fs::read_to_string(filepath)?;
    //     Self::load_any(&content)
    // }
}
