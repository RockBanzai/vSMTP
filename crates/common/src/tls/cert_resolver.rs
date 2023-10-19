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

/// Resolve certificate based on the root and virtual certificate
/// from the service configuration.
pub struct CertResolver {
    pub sni_resolver: rustls::server::ResolvesServerCertUsingSni,
    pub hostname: String,
    pub default_cert: Option<std::sync::Arc<rustls::sign::CertifiedKey>>,
}

impl rustls::server::ResolvesServerCert for CertResolver {
    fn resolve(
        &self,
        client_hello: rustls::server::ClientHello<'_>,
    ) -> Option<std::sync::Arc<rustls::sign::CertifiedKey>> {
        tracing::debug!(
            server_name = ?client_hello.server_name(),
            self.hostname,
            //alpn = client_hello.alpn().map(|b| base64::encode(b)),
            cipher_suites = ?client_hello.cipher_suites(),
            signature_schemes = ?client_hello.signature_schemes(),
            "resolving certificate"
        );
        match client_hello.server_name() {
            Some(server_name) if server_name == self.hostname.as_str() => self.default_cert.clone(),
            Some(_) => self.sni_resolver.resolve(client_hello),
            _ => self.default_cert.clone(),
        }
    }
}
