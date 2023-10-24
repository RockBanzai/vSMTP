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

use super::error::Error;
use vsmtp_auth::{TlsCertificate, TlsPrivateKey};
use vsmtp_protocol::rustls;

/// Certificate and private key for a domain.
#[derive(Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Secret {
    /// Certificate chain to use for the TLS connection.
    /// (the first certificate should certify KEYFILE, the last should be a root CA)
    pub certificate: TlsCertificate,
    /// Private key to use for the TLS connection.
    pub private_key: TlsPrivateKey,
}

impl Secret {
    pub fn to_rustls(&self) -> Result<rustls::sign::CertifiedKey, Error> {
        let key = match self.private_key.private_key() {
            vsmtp_auth::dkim::PrivateKey::Rsa(rsa) => {
                let v = ring_compat::pkcs8::EncodePrivateKey::to_pkcs8_der(rsa.as_ref())?
                    .as_bytes()
                    .to_vec();
                rustls::PrivateKey(v)
            }

            vsmtp_auth::dkim::PrivateKey::Ed25519(_) => {
                // TODO: encode the key to DER for the rustls conversion
                return Err(Error::Ed25519Unimplemented);
            }
        };

        let key = rustls::sign::any_supported_type(&key)?;
        Ok(rustls::sign::CertifiedKey {
            cert: self.certificate.certs().to_vec(),
            key,
            // TODO: support OCSP and SCT
            ocsp: None,
            sct_list: None,
        })
    }
}
