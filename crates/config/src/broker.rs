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
use vsmtp_auth::TlsCertificate;

#[derive(Default, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Broker {
    // FIXME: Which default should be used ?
    /// AMQP endpoint.
    pub uri: String,
    pub extra_root_ca: Option<std::sync::Arc<TlsCertificate>>,
}

impl Broker {
    pub async fn connect(&self) -> Result<lapin::Connection, lapin::Error> {
        let Self { uri, extra_root_ca } = self;

        lapin::Connection::connect_with_config(
            uri,
            lapin::ConnectionProperties::default(),
            lapin::tcp::OwnedTLSConfig {
                identity: None,
                cert_chain: extra_root_ca
                    .as_ref()
                    .map(|certs| certs.source().to_string()),
            },
        )
        .await
    }
}
