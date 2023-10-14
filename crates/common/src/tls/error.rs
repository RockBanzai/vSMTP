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

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("the key format is not supported for this usage")]
    Ed25519UUnimplemented,
    #[error("Failed to sign: {0}")]
    Sign(#[from] rustls::sign::SignError),
    #[error("Pkcs8 format error: {0}")]
    Pkcs8(#[from] ring_compat::pkcs8::Error),
    #[error("No requested TLS versions '{0}' are supported")]
    Versions(String),
    #[error("TLS protocol error: {0}")]
    Protocol(rustls::Error),
    #[error("Certificate path does not exist: {0}")]
    CertificatePath(std::path::PathBuf),
    #[error("Failed to read certificate: {0}")]
    ReadCertificate(std::io::Error),
    #[error("certificate path is valid but the certificate is empty")]
    EmptyCertificate,
    #[error("Private key path does not exist: {0}")]
    PrivateKeyPath(std::path::PathBuf),
    #[error("Failed to read private key: {0}")]
    ReadPrivateKey(std::io::Error),
    #[error(
        "Private key file is valid but vSMTP only support RSA, PKCS8 and EC formats, not '{0}'"
    )]
    UnsupportedPrivateKey(String),
    #[error("private key path is valid but the private key is empty")]
    EmptyPrivateKey,
}
