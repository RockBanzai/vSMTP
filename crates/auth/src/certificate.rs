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

#[derive(Debug, thiserror::Error)]
pub enum TlsCertificateError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("does not contain any certificate")]
    Empty,
}

#[derive(Debug, PartialEq, Eq, serde_with::DeserializeFromStr)]
pub struct TlsCertificate {
    source: Box<str>,
    certs: Vec<rustls::Certificate>,
}

impl serde::Serialize for TlsCertificate {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.source)
    }
}

impl TlsCertificate {
    #[must_use]
    pub const fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn certs(&self) -> &[rustls::Certificate] {
        &self.certs
    }

    fn load_pem(source: &str) -> std::result::Result<Self, TlsCertificateError> {
        let mut reader = std::io::BufReader::new(source.as_bytes());
        let certs = rustls_pemfile::certs(&mut reader)?
            .into_iter()
            .map(rustls::Certificate)
            .collect::<Vec<_>>();

        if certs.is_empty() {
            return Err(TlsCertificateError::Empty);
        }

        Ok(Self {
            source: source.into(),
            certs,
        })
    }

    pub fn load_pem_file(filepath: &str) -> std::result::Result<Self, TlsCertificateError> {
        let source = std::fs::read_to_string(filepath)?;
        Self::load_pem(&source)
    }
}

impl std::str::FromStr for TlsCertificate {
    type Err = TlsCertificateError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::load_pem(s)
    }
}
