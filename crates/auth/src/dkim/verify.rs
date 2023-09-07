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

use super::{BackendError, HashAlgorithm, Mail, PublicKey, Signature, SigningAlgorithm};
use base64::{engine::general_purpose::STANDARD, Engine};

/// Errors that can occur when verifying a DKIM signature
#[must_use]
#[derive(Debug, thiserror::Error)]
pub enum VerifierError {
    /// The signing algorithm is not suitable for the acceptable hash algorithms
    #[error(
        "the `signing_algorithm` ({signing_algorithm}) is not suitable for the `acceptable_hash_algorithms` ({})",
        acceptable
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )]
    AlgorithmMismatch {
        /// The signing algorithm requested
        signing_algorithm: SigningAlgorithm,
        /// The acceptable hash algorithms
        acceptable: Vec<HashAlgorithm>,
    },
    /// The hash algorithm is not supported by the signing algorithm
    #[error(
        "the `signing_algorithm` ({signing_algorithm}) is not suitable for the `acceptable_hash_algorithms` ({})",
        signing_algorithm
            .get_supported_hash_algo()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )]
    HashAlgorithmUnsupported {
        /// The signing algorithm requested
        signing_algorithm: SigningAlgorithm,
    },
    /// The body hash does not match, meaning the message's body has been altered
    #[error(
        "body hash does not match: got: `{}`, expected: `{}`",
        base64::encode(got),
        expected
    )]
    BodyHashMismatch {
        /// The hash produced by the verification
        got: Vec<u8>,
        /// The hash expected
        expected: String,
    },
    /// A base64 error occurred
    #[error("base64 error: {0}")]
    Base64Error(#[from] base64::DecodeError),
    /// The underlying backend returned an error
    #[error("headers hash does not match, got `{0}`")]
    BackendError(#[from] BackendError),
}

/// Verify **ONE** DKIM signature.
///
/// # Errors
///
/// * see [`VerifierError`]
pub fn verify(
    signature: &Signature,
    message: &impl Mail,
    public_key: &PublicKey,
) -> Result<(), VerifierError> {
    if !signature
        .signing_algorithm
        .support_any(&public_key.record.acceptable_hash_algorithms)
    {
        return Err(VerifierError::AlgorithmMismatch {
            signing_algorithm: signature.signing_algorithm,
            acceptable: public_key.record.acceptable_hash_algorithms.clone(),
        });
    }

    let body = signature
        .canonicalization
        .canonicalize_body(&message.get_body());

    #[allow(clippy::option_if_let_else)]
    let body_hash =
        signature
            .signing_algorithm
            .get_preferred_hash_algo()
            .hash(match signature.body_length {
                // TODO: handle policy
                Some(len) => &body[..std::cmp::min(body.len(), len)],
                None => &body,
            });

    if signature.body_hash != STANDARD.encode(&body_hash) {
        return Err(VerifierError::BodyHashMismatch {
            expected: signature.body_hash.clone(),
            got: body_hash,
        });
    }

    let headers_hash = signature.get_header_hash(message);
    tracing::trace!("headers_hash={}", STANDARD.encode(&headers_hash));

    let signature_base64_decoded = STANDARD.decode(&signature.signature)?;

    public_key
        .inner
        .verify(
            &headers_hash,
            &signature_base64_decoded,
            signature.signing_algorithm,
        )
        .map_err(Into::into)
}
