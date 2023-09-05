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

mod algorithm;
mod canonicalization;
mod private_key;
mod public_key;
mod record;
mod signature;

#[cfg(test)]
mod tests {
    mod hash_header;
    // mod sign_verify;
    mod parse {
        mod public_key;
        mod signature_header;
    }
    mod canonicalization;
}

const RSA_MINIMUM_ACCEPTABLE_KEY_SIZE: usize = 1024;

pub use algorithm::{HashAlgorithm, SigningAlgorithm};
pub use canonicalization::Canonicalization;
pub use private_key::PrivateKey;
pub use public_key::PublicKey;
pub use sign::{sign, SigningError};
pub use signature::Signature;
pub use verify::{verify, VerifierError};

#[derive(Debug, thiserror::Error)]
enum BackendError {
    #[error("{0}")]
    Rsa(rsa::errors::Error),
    #[error("{0}")]
    Ed25519(ring_compat::signature::Error),
}

mod verify {
    use super::{BackendError, HashAlgorithm, PublicKey, Signature, SigningAlgorithm};
    use base64::{engine::general_purpose::STANDARD, Engine};
    use vsmtp_mail_parser::Mail;

    #[must_use]
    #[derive(Debug, Default, thiserror::Error)]
    pub(super) enum InnerError {
        #[error(
            "the `signing_algorithm` ({signing_algorithm}) is not suitable for the `acceptable_hash_algorithms` ({})",
            acceptable
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )]
        AlgorithmMismatch {
            signing_algorithm: SigningAlgorithm,
            acceptable: Vec<HashAlgorithm>,
        },
        #[error(
            "the `signing_algorithm` ({signing_algorithm}) is not suitable for the `acceptable_hash_algorithms` ({})",
            signing_algorithm
                .get_supported_hash_algo()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )]
        HashAlgorithmUnsupported { signing_algorithm: SigningAlgorithm },
        #[error(
            "body hash does not match: got: `{}`, expected: `{}`",
            base64::encode(got),
            expected
        )]
        BodyHashMismatch { got: Vec<u8>, expected: String },
        #[error("headers hash does not match, got `{0}`")]
        BackendError(BackendError),
        #[error("base64 error: {error}")]
        Base64Error { error: base64::DecodeError },
        #[default]
        #[error("default invocated")]
        Default,
    }

    ///
    #[derive(Debug, Default)]
    pub struct VerifierError(InnerError);

    impl From<InnerError> for VerifierError {
        fn from(e: InnerError) -> Self {
            Self(e)
        }
    }

    impl std::fmt::Display for VerifierError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    /// Verify **ONE** DKIM signature.
    ///
    /// # Errors
    pub fn verify(
        signature: &Signature,
        message: &Mail,
        public_key: &PublicKey,
    ) -> Result<(), VerifierError> {
        if !signature
            .signing_algorithm
            .support_any(&public_key.record.acceptable_hash_algorithms)
        {
            return Err(InnerError::AlgorithmMismatch {
                signing_algorithm: signature.signing_algorithm,
                acceptable: public_key.record.acceptable_hash_algorithms.clone(),
            }
            .into());
        }

        let body = signature
            .canonicalization
            .canonicalize_body(&message.body.to_string());

        #[allow(clippy::option_if_let_else)]
        let body_hash = signature
            .signing_algorithm
            .get_preferred_hash_algo()
            .hash(match signature.body_length {
                // TODO: handle policy
                Some(len) => &body[..std::cmp::min(body.len(), len)],
                None => &body,
            });

        if signature.body_hash != STANDARD.encode(&body_hash) {
            return Err(InnerError::BodyHashMismatch {
                expected: signature.body_hash.clone(),
                got: body_hash,
            }
            .into());
        }

        let headers_hash = signature.get_header_hash(message);
        tracing::trace!("headers_hash={}", STANDARD.encode(&headers_hash));

        let signature_base64_decoded = STANDARD
            .decode(&signature.signature)
            .map_err(|e| InnerError::Base64Error { error: e })?;

        public_key
            .inner
            .verify(
                &headers_hash,
                &signature_base64_decoded,
                signature.signing_algorithm,
            )
            .map_err(Into::into)
    }
}

mod sign {
    use super::{
        private_key::PrivateKey, signature::QueryMethod, BackendError, Canonicalization, Signature,
        SigningAlgorithm, RSA_MINIMUM_ACCEPTABLE_KEY_SIZE,
    };
    use base64::{engine::general_purpose::STANDARD, Engine};
    use vsmtp_mail_parser::Mail;

    #[must_use]
    #[derive(Debug, thiserror::Error)]
    pub(super) enum InnerError {
        #[error(
            "the `signing_algorithm` ({signing_algorithm}) is not suitable for the `acceptable_hash_algorithms`",
        )]
        HashAlgorithmUnsupported { signing_algorithm: SigningAlgorithm },
        #[error("{0}")]
        BackendError(BackendError),
        #[error(
            "invalid key size: {0} bits, was expecting at least {} bits",
            RSA_MINIMUM_ACCEPTABLE_KEY_SIZE
        )]
        InvalidSize(usize),
    }

    ///
    #[derive(Debug)]
    pub struct SigningError(InnerError);

    impl From<InnerError> for SigningError {
        fn from(e: InnerError) -> Self {
            Self(e)
        }
    }

    impl std::fmt::Display for SigningError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    ///
    ///
    /// # Errors
    pub fn sign(
        message: &Mail,
        private_key: &PrivateKey,
        sdid: String,
        selector: String,
        canonicalization: Canonicalization,
        headers_field: Vec<String>,
        #[cfg(test)] signing_algorithm: Option<SigningAlgorithm>,
        // TODO:
        // auid: String,
        // signature_timestamp: Option<std::time::Duration>,
        // expire_time: Option<std::time::Duration>,
        // body_length: Option<usize>,
        // copy_header_fields: Option<Vec<(String, String)>>,
    ) -> Result<Signature, SigningError> {
        #[cfg(not(test))]
        let signing_algorithm = private_key.get_preferred_signing_algo();
        #[cfg(test)]
        let signing_algorithm =
            signing_algorithm.unwrap_or_else(|| private_key.get_preferred_signing_algo());

        let mut signature = Signature {
            version: 1,
            signing_algorithm,
            sdid,
            selector,
            canonicalization,
            query_method: vec![QueryMethod::default()],
            auid: String::default(),
            signature_timestamp: None,
            expire_time: None,
            body_length: None,
            headers_field,
            copy_header_fields: None,
            body_hash: STANDARD.encode(
                signing_algorithm
                    .get_preferred_hash_algo()
                    .hash(canonicalization.canonicalize_body(&message.body.to_string())),
            ),
            signature: String::default(),
            raw: String::default(),
        };
        signature.raw = signature.to_string();

        signature.signature = STANDARD.encode(private_key.sign(
            signature.signing_algorithm,
            &signature.get_header_hash(message),
        )?);

        signature.raw.push_str(&signature.signature);

        Ok(signature)
    }
}
