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

use super::{sign::SigningError, BackendError, SigningAlgorithm, RSA_MINIMUM_ACCEPTABLE_KEY_SIZE};

/// Private key used for the signature of a message
pub enum PrivateKey {
    /// RSA private key
    Rsa(Box<rsa::RsaPrivateKey>),
    /// Ed25519 private key
    Ed25519(Box<ring_compat::ring::signature::Ed25519KeyPair>),
}

impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rsa(_) => f.debug_struct("Rsa").finish_non_exhaustive(),
            Self::Ed25519(_) => f.debug_struct("Ed25519").finish_non_exhaustive(),
        }
    }
}

impl PartialEq for PrivateKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Rsa(a), Self::Rsa(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for PrivateKey {}

impl PrivateKey {
    pub(super) const fn get_preferred_signing_algo(&self) -> SigningAlgorithm {
        match self {
            Self::Rsa(_) => SigningAlgorithm::RsaSha256,
            Self::Ed25519(_) => SigningAlgorithm::Ed25519Sha256,
        }
    }

    pub(super) fn sign(
        &self,
        signing_algorithm: SigningAlgorithm,
        digest_in: &[u8],
    ) -> Result<Vec<u8>, SigningError> {
        match (self, signing_algorithm) {
            (Self::Rsa(rsa), SigningAlgorithm::RsaSha256) => {
                let size = rsa::traits::PublicKeyParts::size(rsa.as_ref()) * 8;
                if size < RSA_MINIMUM_ACCEPTABLE_KEY_SIZE {
                    return Err(SigningError::InvalidSize(size));
                }
                rsa.sign(rsa::Pkcs1v15Sign::new::<sha2::Sha256>(), digest_in)
                    .map_err(Into::<BackendError>::into)
                    .map_err(Into::into)
            }
            #[cfg(feature = "historic")]
            (Self::Rsa(rsa), SigningAlgorithm::RsaSha1) => rsa
                .sign(rsa::Pkcs1v15Sign::new::<sha1::Sha1>(), digest_in)
                .map_err(Into::<BackendError>::into)
                .map_err(Into::into),
            (Self::Ed25519(ed25519), SigningAlgorithm::Ed25519Sha256) => {
                Ok(ed25519.sign(digest_in).as_ref().to_vec())
            }
            _ => Err(SigningError::HashAlgorithmUnsupported { signing_algorithm }),
        }
    }
}
