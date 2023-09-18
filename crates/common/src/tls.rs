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

mod cert_resolver;
mod cipher_suite;
pub mod error;
mod logger;
pub mod protocol_version;
pub mod secret;

pub use cert_resolver::CertResolver;
pub use cipher_suite::CipherSuite;
pub use protocol_version::ProtocolVersion;

use self::secret::Secret;
use error::Error;
use vsmtp_protocol::{rustls, Domain};

static JUST_TLS1_2: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS12];
static JUST_TLS1_3: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];
/// All TLS version which are not deprecated.
static ALL_VERSIONS: &[&rustls::SupportedProtocolVersion] =
    &[&rustls::version::TLS13, &rustls::version::TLS12];

/// TLS data exchanged during the handshake.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TlsProps {
    /// Version of TLS used for the transaction.
    pub protocol_version: ProtocolVersion,
    /// Cipher suite used for the transaction.
    pub cipher_suite: CipherSuite,
    /// Certificate chain used by the peer to authenticate.
    #[serde(
        serialize_with = "serde_with::As::<Option<Vec<serde_with::base64::Base64>>>::serialize",
        deserialize_with = "TlsProps::deserialize"
    )]
    pub peer_certificates: Option<Vec<rustls::Certificate>>,
    /// Protocol used by the server and peer, established via ALPN.
    pub alpn_protocol: Option<Vec<u8>>,
}

impl TlsProps {
    fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<rustls::Certificate>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <Option<Vec<String>> as serde::Deserialize>::deserialize(deserializer)?
            .map(|certs| {
                match certs
                    .into_iter()
                    .map(|i| rustls_pemfile::certs(&mut i.as_bytes()))
                    .collect::<Result<Vec<Vec<Vec<u8>>>, _>>()
                {
                    Ok(certs) => Ok(certs
                        .into_iter()
                        .flatten()
                        .map(rustls::Certificate)
                        .collect()),
                    Err(e) => Err(serde::de::Error::custom(e)),
                }
            })
            .transpose()
    }
}

impl<T> fake::Dummy<T> for TlsProps {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_config: &T, _rng: &mut R) -> Self {
        todo!()
    }
}

/// Generate rustls configuration from the smtp receiver configuration.
pub fn get_rustls_config(
    protocol_version: &[ProtocolVersion],
    cipher_suite: &[CipherSuite],
    preempt_cipherlist: bool,
    hostname: &str,
    root: Option<&Secret>,
    r#virtual: &std::collections::BTreeMap<Domain, Secret>,
) -> Result<rustls::ServerConfig, Error> {
    fn to_rustls(
        cert: Vec<rustls::Certificate>,
        key: &rustls::PrivateKey,
    ) -> Result<rustls::sign::CertifiedKey, Error> {
        rustls::sign::any_supported_type(key)
            .map_err(Error::Sign)
            .map(|key| {
                rustls::sign::CertifiedKey {
                    cert,
                    key,
                    // TODO: support OCSP and SCT
                    ocsp: None,
                    sct_list: None,
                }
            })
    }

    let protocol_version = match (
        protocol_version
            .iter()
            .any(|i| i.0 == rustls::ProtocolVersion::TLSv1_2),
        protocol_version
            .iter()
            .any(|i| i.0 == rustls::ProtocolVersion::TLSv1_3),
    ) {
        (true, true) => ALL_VERSIONS,
        (true, false) => JUST_TLS1_2,
        (false, true) => JUST_TLS1_3,
        (false, false) => {
            return Err(Error::Versions(
                protocol_version
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            ))
        }
    };

    let mut cert_resolver = rustls::server::ResolvesServerCertUsingSni::new();

    for (domain, secret) in r#virtual {
        cert_resolver
            .add(
                &domain.to_string(),
                to_rustls(
                    secret.certificate.inner.clone(),
                    &secret.private_key.inner.clone(),
                )?,
            )
            .map_err(Error::Protocol)?;
    }

    let mut tls_config = rustls::ServerConfig::builder()
        .with_cipher_suites(&to_supported_cipher_suite(cipher_suite))
        .with_kx_groups(&rustls::ALL_KX_GROUPS)
        .with_protocol_versions(protocol_version)
        .map_err(Error::Protocol)?
        .with_client_cert_verifier(rustls::server::NoClientAuth::boxed())
        .with_cert_resolver(std::sync::Arc::new(cert_resolver::CertResolver {
            sni_resolver: cert_resolver,
            hostname: hostname.to_string(),
            default_cert: root
                .as_ref()
                .map(|secret| {
                    to_rustls(
                        secret.certificate.inner.clone(),
                        &secret.private_key.inner.clone(),
                    )
                })
                .transpose()?
                .map(std::sync::Arc::new),
        }));

    tls_config.ignore_client_order = preempt_cipherlist;
    tls_config.key_log = std::sync::Arc::new(logger::TlsLogger);

    Ok(tls_config)
}

fn to_supported_cipher_suite(cipher_suite: &[CipherSuite]) -> Vec<rustls::SupportedCipherSuite> {
    rustls::ALL_CIPHER_SUITES
        .iter()
        .filter(|i| cipher_suite.iter().any(|x| x.0 == i.suite()))
        .copied()
        .collect::<Vec<_>>()
}
