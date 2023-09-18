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

/// Certificate and private key for a domain.
#[derive(Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Secret {
    /// Certificate chain to use for the TLS connection.
    /// (the first certificate should certify KEYFILE, the last should be a root CA)
    pub certificate: SecretFile<Vec<rustls::Certificate>>,
    /// Private key to use for the TLS connection.
    pub private_key: SecretFile<rustls::PrivateKey>,
}

use super::error::Error;
use vsmtp_protocol::rustls;

/// Abstraction to de/serialize certificates and private key.
/// Also prevent those to be leaked in metadata or logs.
#[doc(hidden)]
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(transparent, deny_unknown_fields)]
pub struct SecretFile<T> {
    #[serde(skip_serializing)]
    pub inner: T,
    pub path: std::path::PathBuf,
}

impl<'de> serde::Deserialize<'de> for SecretFile<Vec<vsmtp_protocol::rustls::Certificate>> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self {
            inner: tls_certificate_from_path(&s).map_err(serde::de::Error::custom)?,
            path: s.into(),
        })
    }
}

/// Read a certificate from file.
pub fn tls_certificate_from_path(path: &str) -> Result<Vec<rustls::Certificate>, Error> {
    let path = std::path::Path::new(&path);

    if !path.exists() {
        return Err(Error::CertificatePath(path.to_path_buf()));
    }

    tls_certificate_from_string(&std::fs::read_to_string(path).map_err(Error::ReadCertificate)?)
}

/// Read a certificate from a string.
pub fn tls_certificate_from_string(input: &str) -> Result<Vec<rustls::Certificate>, Error> {
    let mut reader = std::io::BufReader::new(input.as_bytes());

    let pem = rustls_pemfile::certs(&mut reader)
        .map_err(Error::ReadCertificate)?
        .into_iter()
        .map(rustls::Certificate)
        .collect::<Vec<_>>();

    if pem.is_empty() {
        return Err(Error::EmptyCertificate);
    }

    Ok(pem)
}

impl<'de> serde::Deserialize<'de> for SecretFile<vsmtp_protocol::rustls::PrivateKey> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self {
            inner: tls_private_key_from_path(&s).map_err(serde::de::Error::custom)?,
            path: s.into(),
        })
    }
}

/// Read a private key from a path.
pub fn tls_private_key_from_path(input: &str) -> Result<rustls::PrivateKey, Error> {
    let path = std::path::Path::new(input);

    if !path.exists() {
        return Err(Error::PrivateKeyPath(path.to_path_buf()));
    }

    tls_private_key_from_string(&std::fs::read_to_string(input).map_err(Error::ReadPrivateKey)?)
}

/// Read a private key from a string.
pub fn tls_private_key_from_string(input: &str) -> Result<rustls::PrivateKey, Error> {
    let mut reader = std::io::BufReader::new(input.as_bytes());

    let pem = rustls_pemfile::read_one(&mut reader)
        .map_err(Error::ReadPrivateKey)?
        .into_iter()
        .map(|format| match format {
            rustls_pemfile::Item::RSAKey(key)
            | rustls_pemfile::Item::PKCS8Key(key)
            | rustls_pemfile::Item::ECKey(key) => Ok(rustls::PrivateKey(key)),
            rustls_pemfile::Item::X509Certificate(_) => {
                Err(Error::UnsupportedPrivateKey("X509".to_string()))
            }
            rustls_pemfile::Item::Crl(_) => Err(Error::UnsupportedPrivateKey("Crl".to_string())),
            // Due to non-exhaustive enum.
            _ => Err(Error::UnsupportedPrivateKey("unknown".to_string())),
        })
        .collect::<Result<Vec<_>, Error>>()?;

    pem.first().cloned().ok_or_else(|| Error::EmptyPrivateKey)
}

#[cfg(test)]
mod tests {
    use super::SecretFile;
    use vsmtp_protocol::rustls;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct S {
        v: SecretFile<Vec<rustls::Certificate>>,
    }

    #[ignore = "Need arbitrary certificates to test serialization"]
    #[test]
    fn basic() {
        // let _droppable = std::fs::DirBuilder::new().create("./tmp");

        // let mut file = std::fs::OpenOptions::new()
        //     .create(true)
        //     .write(true)
        //     .open("./tmp/crt")
        //     .unwrap();
        // file.write_all(get_tls_file::get_certificate().as_bytes())
        //     .unwrap();

        // serde_json::from_str::<S>(r#"{"v": "./tmp/crt"}"#).unwrap();
    }

    #[test]
    fn not_a_string() {
        serde_json::from_str::<S>(r#"{"v": 10}"#).unwrap_err();
    }

    #[test]
    fn not_valid_path() {
        serde_json::from_str::<S>(r#"{"v": "foobar"}"#).unwrap_err();
    }
}

#[cfg(test)]
mod tests2 {
    use super::SecretFile;
    use vsmtp_protocol::rustls;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct S {
        v: SecretFile<rustls::PrivateKey>,
    }

    #[ignore = "Need arbitrary certificates to test serialization"]
    #[test]
    fn rsa_ok() {
        // let _droppable = std::fs::DirBuilder::new().create("./tmp");

        // let mut file = std::fs::OpenOptions::new()
        //     .create(true)
        //     .write(true)
        //     .open("./tmp/rsa_key")
        //     .unwrap();
        // file.write_all(get_tls_file::get_rsa_key().as_bytes())
        //     .unwrap();

        // serde_json::from_str::<S>(r#"{"v": "./tmp/rsa_key"}"#).unwrap();
    }

    #[ignore = "Need arbitrary certificates to test serialization"]
    #[test]
    fn pkcs8_ok() {
        // let _droppable = std::fs::DirBuilder::new().create("./tmp");

        // let mut file = std::fs::OpenOptions::new()
        //     .create(true)
        //     .write(true)
        //     .open("./tmp/pkcs8_key")
        //     .unwrap();
        // file.write_all(get_tls_file::get_pkcs8_key().as_bytes())
        //     .unwrap();

        // serde_json::from_str::<S>(r#"{"v": "./tmp/pkcs8_key"}"#).unwrap();
    }

    #[ignore = "Need arbitrary certificates to test serialization"]
    #[test]
    fn ec256_ok() {
        // let _droppable = std::fs::DirBuilder::new().create("./tmp");

        // let mut file = std::fs::OpenOptions::new()
        //     .create(true)
        //     .write(true)
        //     .open("./tmp/ec256_key")
        //     .unwrap();
        // file.write_all(get_tls_file::get_ec256_key().as_bytes())
        //     .unwrap();

        // serde_json::from_str::<S>(r#"{"v": "./tmp/ec256_key"}"#).unwrap();
    }

    #[ignore = "Need arbitrary certificates to test serialization"]
    #[test]
    fn not_good_format() {
        // let _droppable = std::fs::DirBuilder::new().create("./tmp");

        // let mut file = std::fs::OpenOptions::new()
        //     .create(true)
        //     .write(true)
        //     .open("./tmp/crt2")
        //     .unwrap();
        // file.write_all(get_tls_file::get_certificate().as_bytes())
        //     .unwrap();

        // serde_json::from_str::<S>(r#"{"v": "./tmp/crt2"}"#).unwrap_err();
    }

    #[test]
    fn not_a_string() {
        serde_json::from_str::<S>(r#"{"v": 10}"#).unwrap_err();
    }

    #[test]
    fn not_valid_path() {
        serde_json::from_str::<S>(r#"{"v": "foobar"}"#).unwrap_err();
    }
}
