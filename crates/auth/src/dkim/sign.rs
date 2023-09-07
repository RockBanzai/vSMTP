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

use super::{
    private_key::PrivateKey, signature::QueryMethod, BackendError, Canonicalization, Mail,
    Signature, SigningAlgorithm, RSA_MINIMUM_ACCEPTABLE_KEY_SIZE,
};
use base64::{engine::general_purpose::STANDARD, Engine};

/// Error that can occur during the signature of a message
#[must_use]
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    /// The signing algorithm is not suitable for the acceptable hash algorithms
    #[error(
        "the `signing_algorithm` ({signing_algorithm}) is not suitable for the `acceptable_hash_algorithms`",
    )]
    HashAlgorithmUnsupported {
        /// The signing algorithm requested
        signing_algorithm: SigningAlgorithm,
    },
    /// The key size is too small
    #[error(
        "invalid key size: {0} bits, was expecting at least {} bits",
        RSA_MINIMUM_ACCEPTABLE_KEY_SIZE
    )]
    InvalidSize(usize),
    /// The underlying backend returned an error
    #[error("{0}")]
    BackendError(#[from] BackendError),
}

///
///
/// # Errors
pub fn sign(
    message: &impl Mail,
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
                .hash(canonicalization.canonicalize_body(&message.get_body())),
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
