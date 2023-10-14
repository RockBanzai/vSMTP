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

/// Hash & sign algorithm exposed in a `DKIM-Signature` header. Used by the
/// expose the algorithm used to verify the message.
#[allow(clippy::module_name_repetitions)]
#[derive(
    Debug,
    PartialEq,
    Eq,
    Copy,
    Clone,
    strum::EnumString,
    strum::Display,
    serde_with::SerializeDisplay,
    serde_with::DeserializeFromStr,
    fake::Dummy,
)]
pub enum SigningAlgorithm {
    /// The SHA-1 hash function should be considered cryptographically broken and unsuitable
    /// for further use in any security critical capacity.
    ///
    /// See the implementation <https://docs.rs/sha1>
    #[cfg_attr(docsrs, doc(cfg(feature = "historic")))]
    #[cfg(feature = "historic")]
    #[strum(serialize = "rsa-sha1")]
    RsaSha1,
    /// See the implementation <https://docs.rs/sha2>
    #[strum(serialize = "rsa-sha256")]
    RsaSha256,
    /// See the implementation <https://docs.rs/ring-compat/0.7.0/ring_compat/signature/ed25519/index.html>
    #[strum(serialize = "ed25519-sha256")]
    Ed25519Sha256,
}

impl SigningAlgorithm {
    pub(super) fn support_any(self, hash_algo: &[HashAlgorithm]) -> bool {
        let supported = self.get_supported_hash_algo();
        hash_algo.iter().any(|a| supported.contains(a))
    }

    pub(super) const fn get_supported_hash_algo(self) -> &'static [HashAlgorithm] {
        match self {
            #[cfg(feature = "historic")]
            Self::RsaSha1 => &[HashAlgorithm::Sha1],
            #[cfg(feature = "historic")]
            Self::RsaSha256 => &[HashAlgorithm::Sha256, HashAlgorithm::Sha1],
            _ => &[HashAlgorithm::Sha256],
        }
    }

    pub(super) fn get_preferred_hash_algo(self) -> &'static HashAlgorithm {
        self.get_supported_hash_algo()
            .first()
            .expect("has at least one algorithm")
    }
}

/// Hash algorithms exposed in the `DKIM record`,
/// used to describe the content of the "p=" tag in the record.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum HashAlgorithm {
    /// The SHA-1 hash function should be considered cryptographically broken and unsuitable
    /// for further use in any security critical capacity.
    ///
    /// See the implementation <https://docs.rs/sha1>
    #[cfg_attr(docsrs, doc(cfg(feature = "historic")))]
    #[cfg(feature = "historic")]
    Sha1,
    /// See the implementation <https://docs.rs/sha2>
    Sha256,
}

impl HashAlgorithm {
    /// Return the hashed `data` using the algorithm.
    #[must_use]
    pub fn hash<T: AsRef<[u8]>>(self, data: T) -> Vec<u8> {
        match self {
            #[cfg(feature = "historic")]
            Self::Sha1 => {
                let mut digest = <sha1::Sha1 as sha1::Digest>::new();
                sha1::Digest::update(&mut digest, data);
                sha1::Digest::finalize(digest).to_vec()
            }
            Self::Sha256 => {
                let mut digest = <sha2::Sha256 as sha2::Digest>::new();
                sha2::Digest::update(&mut digest, data);
                sha2::Digest::finalize(digest).to_vec()
            }
        }
    }
}
